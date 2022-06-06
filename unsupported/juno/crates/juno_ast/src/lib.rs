/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Juno abstract syntax tree.
//!
//! Provides a transformable AST which is stored in a garbage-collected heap.
//! All nodes are stored in [`Context`], which handles memory management of the nodes
//! and exposes a safe API.
//!
//! Allocation and viewing of nodes must be done via the use of a [`GCLock`],
//! **of which there must be only one active per thread at any time**,
//! to avoid accidentally mixing `Node`s between `Context`.
//! The `GCLock` will provide `&'gc Node<'gc>`,
//! i.e. a `Node` that does not outlive the `GCLock` and which references other `Node`s which
//! also do not outlive the `GCLock`.
//!
//! Nodes are allocated and cloned/modified by using the various `Builder` structs,
//! for example [`NumericLiteralBuilder`].
//! Builder structs have `build_template` functions that take "template" structs,
//! which have the same general structure as the various node kinds, but are only used
//! for building/allocating nodes in the `Context`.
//!
//! Visitor patterns are provided by [`Visitor`] and [`VisitorMut`].

use context::NodeListElement;
use juno_support::atom_table::{Atom, AtomU16};
use juno_support::define_str_enum;
use std::marker::PhantomData;
use thiserror::Error;

#[macro_use]
mod def;

mod context;
mod dump;
mod field;
mod kind;
mod validate;

pub use juno_support::source_manager::{SourceId, SourceLoc, SourceManager, SourceRange};

pub use field::NodeField;
pub use kind::NodeVariant;

pub use context::{Context, GCLock, NodePtr, NodeRc};
pub use dump::{dump_json, Pretty};
pub use kind::*;
pub use validate::{validate_tree, validate_tree_pure, TreeValidationError, ValidationError};

/// Indicates the path to the current node.
#[derive(Debug, Copy, Clone)]
pub struct Path<'a> {
    /// Parent node.
    pub parent: &'a Node<'a>,

    /// Field name in the path node.
    pub field: NodeField,
}

impl<'a> Path<'a> {
    pub fn new(parent: &'a Node<'a>, field: NodeField) -> Path<'a> {
        Path { parent, field }
    }
}

/// Trait implemented by those who call the visit functionality.
pub trait Visitor<'gc> {
    /// Visit the Node `node` with the given `path`.
    fn call(&mut self, ctx: &'gc GCLock, node: &'gc Node<'gc>, path: Option<Path<'gc>>);
}

/// Indicates what mutation occurred to an element of the AST during [`VisitorMut`] use.
#[derive(Debug)]
pub enum TransformResult<T> {
    /// No change to the element.
    Unchanged,

    /// Element must be removed, if possible.
    /// If the element cannot be removed, it will be replaced with EmptyStatement.
    /// **Only statements may be removed from non-optional fields,
    /// as removing expressions would result in an invalid AST.**
    Removed,

    /// Element should be swapped out for the wrapped element.
    Changed(T),

    /// Element should be swapped out for multiple wrapped elements.
    Expanded(Vec<T>),
}

/// Trait implemented by those who call the visit functionality.
pub trait VisitorMut<'gc> {
    /// Visit the Node `node` with the given `path`.
    fn call(
        &mut self,
        ctx: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>>;
}

/// Metadata common to all AST nodes.
///
/// Stored inside [`Node`] and must not be constructed directly by users.
/// Instead it should be allocated in the GC by using the `Builder`.
/// Must not derive `Clone` or `Copy` in order to avoid `Node` being allocated
/// by callers outside this module.
#[derive(Debug)]
pub struct NodeMetadata<'a> {
    phantom: PhantomData<&'a Node<'a>>,
    pub range: SourceRange,
}

impl<'a> NodeMetadata<'a> {
    fn build_template(template: TemplateMetadata<'a>) -> NodeMetadata<'a> {
        NodeMetadata {
            phantom: template.phantom,
            range: template.range,
        }
    }
}

/// Metadata common to all AST nodes used in templates.
/// Stored inside template notes and may be constructed directly by users.
#[derive(Debug, Clone)]
pub struct TemplateMetadata<'a> {
    pub phantom: PhantomData<&'a Node<'a>>,
    pub range: SourceRange,
}

impl Default for TemplateMetadata<'_> {
    fn default() -> Self {
        Self {
            phantom: Default::default(),
            range: SourceRange {
                file: SourceId::INVALID,
                start: SourceLoc::invalid(),
                end: SourceLoc::invalid(),
            },
        }
    }
}

impl<'a> From<&NodeMetadata<'a>> for TemplateMetadata<'a> {
    fn from(metadata: &NodeMetadata<'a>) -> Self {
        Self {
            phantom: Default::default(),
            range: metadata.range,
        }
    }
}

impl<'a> From<SourceRange> for TemplateMetadata<'a> {
    fn from(range: SourceRange) -> Self {
        Self {
            phantom: Default::default(),
            range,
        }
    }
}

/// JS identifier represented as valid UTF-8.
pub type NodeLabel = Atom;

/// An ordered list of nodes used as a property in the AST.
///
/// Implemented as a linked list internally to avoid extra overhead that would exist if it were
/// to allocate a `Vec` or some other structure that required allocating on the native heap.
///
/// Because this is just a pointer to the head of the list, it implements `Copy` much like any other
/// pointer/reference, allowing to user to handle it much like `&Node` in many cases.
#[derive(Debug, Copy, Clone)]
pub struct NodeList<'a> {
    /// If non-null, pointer to the first element of the list.
    /// If null, the list is empty.
    head: *const NodeListElement<'a>,
}

impl<'a> NodeList<'a> {
    /// Create a new empty list.
    /// Guaranteed to be fast, performs no allocations.
    pub fn new<'gc>(_: &'gc GCLock) -> NodeList<'gc> {
        NodeList {
            head: std::ptr::null(),
        }
    }

    /// Connect the provided pre-existing nodes into a `NodeList` via iteration.
    /// `NodeList` doesn't implement `FromIterator` directly due to the `GCLock` requirement.
    pub fn from_iter<'gc, I: IntoIterator<Item = &'gc Node<'gc>>>(
        lock: &'gc GCLock<'_, '_>,
        nodes: I,
    ) -> NodeList<'gc> {
        let mut it = nodes.into_iter();
        match it.next() {
            Some(first) => {
                // At least one element in the list.
                // Allocate the `NodeListElement`s in the context.
                let head_elem: &'gc NodeListElement<'gc> = lock.append_list_element(None, first);
                let mut prev_elem = head_elem;
                // Exhaust the rest of the iterator.
                for next in it {
                    let next_elem = lock.append_list_element(Some(prev_elem), next);
                    prev_elem = next_elem;
                }
                NodeList { head: head_elem }
            }
            _ => {
                // No elements, return the empty `NodeList`.
                Self::new(lock)
            }
        }
    }

    pub fn iter(self) -> NodeListIterator<'a> {
        NodeListIterator { ptr: self.head }
    }

    /// Whether this `NodeList` has no elements.
    /// Cost: `O(1)`
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Return the first element of the list if it exists, else `None`.
    pub fn head(&self) -> Option<&'a Node<'a>> {
        self.iter().next()
    }

    /// Return the length of the list.
    /// Cost: `O(n)` where `n` is the length of the list.
    /// Use [`Self::is_empty`] instead if you just want to check whether it's empty.
    pub fn len(&self) -> usize {
        self.iter().count()
    }
}

impl<'a> IntoIterator for NodeList<'a> {
    type Item = &'a Node<'a>;

    type IntoIter = NodeListIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator for `Node`s in the `NodeList`.
pub struct NodeListIterator<'a> {
    /// The upcoming element in the iteration order.
    /// `null` if the iteration is complete (`next` will return `None`).
    ptr: *const NodeListElement<'a>,
}

impl<'a> Iterator for NodeListIterator<'a> {
    type Item = &'a Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr.is_null() {
            None
        } else {
            unsafe {
                let result = &*self.ptr;
                self.ptr = result.next.get();
                debug_assert!(!result.inner.is_null(), "NodeList node must not be null");
                Some(&*result.inner)
            }
        }
    }
}

/// JS string literals don't have to contain valid UTF-8,
/// so we wrap a `Vec<u16>`, which allows us to represent UTF-16 characters
/// without being subject to Rust's restrictions on [`String`].
pub type NodeString = AtomU16;

#[derive(Debug, Copy, Clone, Error)]
#[error("Invalid string property for AST node")]
pub struct TryFromStringError;

define_str_enum!(
    UnaryExpressionOperator,
    TryFromStringError,
    (Delete, "delete"),
    (Void, "void"),
    (Typeof, "typeof"),
    (Plus, "+"),
    (Minus, "-"),
    (BitNot, "~"),
    (Not, "!"),
);

define_str_enum!(
    BinaryExpressionOperator,
    TryFromStringError,
    (LooseEquals, "=="),
    (LooseNotEquals, "!="),
    (StrictEquals, "==="),
    (StrictNotEquals, "!=="),
    (Less, "<"),
    (LessEquals, "<="),
    (Greater, ">"),
    (GreaterEquals, ">="),
    (LShift, "<<"),
    (RShift, ">>"),
    (RShift3, ">>>"),
    (Plus, "+"),
    (Minus, "-"),
    (Mult, "*"),
    (Div, "/"),
    (Mod, "%"),
    (BitOr, "|"),
    (BitXor, "^"),
    (BitAnd, "&"),
    (Exp, "**"),
    (In, "in"),
    (Instanceof, "instanceof"),
);

define_str_enum!(
    LogicalExpressionOperator,
    TryFromStringError,
    (And, "&&"),
    (Or, "||"),
    (NullishCoalesce, "??"),
);

define_str_enum!(
    UpdateExpressionOperator,
    TryFromStringError,
    (Increment, "++"),
    (Decrement, "--"),
);

define_str_enum!(
    AssignmentExpressionOperator,
    TryFromStringError,
    (Assign, "="),
    (LShiftAssign, "<<="),
    (RShiftAssign, ">>="),
    (RShift3Assign, ">>>="),
    (PlusAssign, "+="),
    (MinusAssign, "-="),
    (MultAssign, "*="),
    (DivAssign, "/="),
    (ModAssign, "%="),
    (BitOrAssign, "|="),
    (BitXorAssign, "^="),
    (BitAndAssign, "&="),
    (ExpAssign, "**="),
    (LogicalOrAssign, "||="),
    (LogicalAndAssign, "&&="),
    (NullishCoalesceAssign, "??="),
);

define_str_enum!(
    VariableDeclarationKind,
    TryFromStringError,
    (Var, "var"),
    (Let, "let"),
    (Const, "const"),
);

define_str_enum!(
    PropertyKind,
    TryFromStringError,
    (Init, "init"),
    (Get, "get"),
    (Set, "set"),
);

define_str_enum!(
    MethodDefinitionKind,
    TryFromStringError,
    (Method, "method"),
    (Constructor, "constructor"),
    (Get, "get"),
    (Set, "set"),
);

define_str_enum!(
    ImportKind,
    TryFromStringError,
    (Value, "value"),
    (Type, "type"),
    (Typeof, "typeof"),
);

define_str_enum!(
    ExportKind,
    TryFromStringError,
    (Value, "value"),
    (Type, "type"),
);

impl<'gc> Node<'gc> {
    pub fn visit<V: Visitor<'gc>>(
        &'gc self,
        ctx: &'gc GCLock,
        visitor: &mut V,
        path: Option<Path<'gc>>,
    ) {
        visitor.call(ctx, self, path);
    }

    /// Visit this node with `visitor` and return the modified root node.
    /// If the root node is to be removed, return `None`.
    pub fn visit_mut<V: VisitorMut<'gc>>(
        &'gc self,
        ctx: &'gc GCLock,
        visitor: &mut V,
        path: Option<Path<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        match visitor.call(ctx, self, path) {
            TransformResult::Unchanged => Some(self),
            TransformResult::Removed => None,
            TransformResult::Changed(new_node) => Some(new_node),
            TransformResult::Expanded(_) => {
                panic!("Attempt to replace a single node with multiple");
            }
        }
    }
}

/// Trait implemented by possible child types of `NodeKind`.
trait NodeChild<'gc>
where
    Self: std::marker::Sized,
{
    type Out;

    /// Visit this child of the given `node`.
    /// Should be no-op for any type that doesn't contain pointers to other
    /// `Node`s.
    fn visit_child<V: Visitor<'gc>>(self, _ctx: &'gc GCLock, _visitor: &mut V, _path: Path<'gc>) {}

    /// Visit this child of the given `node`.
    /// Should be no-op for any type that doesn't contain pointers to other
    /// `Node`s.
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        _ctx: &'gc GCLock,
        _visitor: &mut V,
        _path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        TransformResult::Unchanged
    }

    /// A way to "clone" children without actually implementing Clone.
    /// Not implementing Clone prevents accidental creation of `Node` references
    /// on the stack.
    /// Can't provide a default implementation here because associated type default definitions
    /// (for `Out`) are unstable.
    fn duplicate(self) -> Self::Out;

    /// If this NodeChild is a list, visit the elements and call `cb` with each `NodeListElement`
    /// in this AST node only (non-recursive).
    fn mark_list<CB: Fn(&NodeListElement)>(self, _lock: &'gc GCLock, _cb: CB) {}
}

impl NodeChild<'_> for f64 {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for bool {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for NodeLabel {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for UnaryExpressionOperator {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for BinaryExpressionOperator {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for LogicalExpressionOperator {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for UpdateExpressionOperator {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for AssignmentExpressionOperator {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for VariableDeclarationKind {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for PropertyKind {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for MethodDefinitionKind {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for ImportKind {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for ExportKind {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl NodeChild<'_> for NodeString {
    type Out = Self;
    fn duplicate(self) -> Self::Out {
        self
    }
}
impl<'gc> NodeChild<'gc> for &NodeString {
    type Out = NodeString;
    fn duplicate(self) -> Self::Out {
        *self
    }
}
impl NodeChild<'_> for &Option<NodeString> {
    type Out = Option<NodeString>;
    fn duplicate(self) -> Self::Out {
        *self
    }
}

impl<'gc, T: NodeChild<'gc> + NodeChild<'gc, Out = T>> NodeChild<'gc> for Option<T> {
    type Out = Self;

    fn visit_child<V: Visitor<'gc>>(self, ctx: &'gc GCLock, visitor: &mut V, path: Path<'gc>) {
        if let Some(t) = self {
            t.visit_child(ctx, visitor, path);
        }
    }

    /// Visit this child of the given `node`.
    /// Should be no-op for any type that doesn't contain pointers to other
    /// `Node`s.
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        use TransformResult::*;
        match self {
            None => Unchanged,
            Some(inner) => match inner.visit_child_mut(ctx, visitor, path) {
                Unchanged => Unchanged,
                Removed => Changed(None),
                Changed(new_node) => Changed(Some(new_node)),
                Expanded(_) => {
                    panic!("Attempt to replace a single optional node with multiple");
                }
            },
        }
    }

    fn duplicate(self) -> Self::Out {
        self.map(|inner| inner.duplicate())
    }
}

impl<'gc> NodeChild<'gc> for &Option<&'gc Node<'gc>> {
    type Out = Option<&'gc Node<'gc>>;

    fn visit_child<V: Visitor<'gc>>(self, ctx: &'gc GCLock, visitor: &mut V, path: Path<'gc>) {
        if let Some(t) = *self {
            t.visit_child(ctx, visitor, path);
        }
    }

    /// Visit this child of the given `node`.
    /// Should be no-op for any type that doesn't contain pointers to other
    /// `Node`s.
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        use TransformResult::*;
        match self {
            None => Unchanged,
            Some(inner) => match inner.visit_child_mut(ctx, visitor, path) {
                Unchanged => Unchanged,
                Removed => Changed(None),
                Changed(new_node) => Changed(Some(new_node)),
                Expanded(_) => {
                    panic!("Attempt to replace a single optional node with multiple");
                }
            },
        }
    }

    fn duplicate(self) -> Self::Out {
        *self
    }
}

impl<'gc> NodeChild<'gc> for &'gc Node<'gc> {
    type Out = Self;

    fn visit_child<V: Visitor<'gc>>(self, ctx: &'gc GCLock, visitor: &mut V, path: Path<'gc>) {
        visitor.call(ctx, self, Some(path));
    }

    /// Visit this child of the given `node`.
    /// Should be no-op for any type that doesn't contain pointers to other
    /// `Node`s.
    fn visit_child_mut<V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        match visitor.call(ctx, self, Some(path)) {
            TransformResult::Removed => {
                TransformResult::Changed(builder::EmptyStatement::build_template(
                    ctx,
                    template::EmptyStatement {
                        metadata: TemplateMetadata {
                            phantom: Default::default(),
                            range: SourceRange {
                                file: self.range().file,
                                start: self.range().start,
                                end: self.range().start,
                            },
                        },
                    },
                ))
            }
            result => result,
        }
    }

    fn duplicate(self) -> Self::Out {
        self
    }
}

impl<'gc> NodeChild<'gc> for NodeList<'gc> {
    type Out = NodeList<'gc>;

    fn visit_child<V: Visitor<'gc>>(self, ctx: &'gc GCLock, visitor: &mut V, path: Path<'gc>) {
        for child in self.iter() {
            visitor.call(ctx, child, Some(path));
        }
    }

    fn visit_child_mut<'ast, 'ctx, V: VisitorMut<'gc>>(
        self,
        ctx: &'gc GCLock<'ast, 'ctx>,
        visitor: &mut V,
        path: Path<'gc>,
    ) -> TransformResult<Self::Out> {
        use TransformResult::*;
        let mut index = 0;
        let mut it: NodeListIterator<'gc> = self.iter();
        // Assume no copies to start.
        while let Some(elem) = it.next() {
            let node = visitor.call(ctx, elem, Some(path));
            if let Unchanged = node {
                index += 1;
                continue;
            }
            // First change found, either removed or changed node.
            // Fill in the elements we skippped.
            let mut result = self.iter().take(index).collect::<Vec<&Node>>();
            // If the node was changed or expanded, push it.
            match node {
                Changed(new_node) => result.push(new_node),
                Expanded(new_nodes) => {
                    result.extend(new_nodes);
                }
                Removed => {}
                Unchanged => {
                    unreachable!("checked for unchanged above")
                }
            };
            index += 1;
            // Fill the rest of the elements.
            for elem in it.by_ref() {
                match visitor.call(ctx, elem, Some(path)) {
                    Unchanged => result.push(elem),
                    Removed => {}
                    Changed(new_node) => result.push(new_node),
                    Expanded(new_nodes) => {
                        result.extend(new_nodes);
                    }
                }
                index += 1;
            }
            return Changed(NodeList::from_iter(ctx, result));
        }
        Unchanged
    }

    fn duplicate(self) -> Self::Out {
        NodeList { head: self.head }
    }

    fn mark_list<CB: Fn(&NodeListElement)>(self, _lock: &'gc GCLock, cb: CB) {
        let mut cur = self.head;
        while !cur.is_null() {
            let elem = unsafe { &*cur };
            cb(elem);
            cur = elem.next.get();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_node_ref() {
        let mut ctx = Context::new();
        let lock = GCLock::new(&mut ctx);

        let mut m = HashMap::new();

        let n1 = NodePtr::from(builder::NumericLiteral::build_template(
            &lock,
            template::NumericLiteral {
                metadata: Default::default(),
                value: 10.0,
            },
        ));
        let n2 = NodePtr::from(builder::NumericLiteral::build_template(
            &lock,
            template::NumericLiteral {
                metadata: Default::default(),
                value: 20.0,
            },
        ));
        let n25 = Box::new(n2);
        assert_ne!(n1, n2);
        assert_eq!(n2, *n25);
        m.insert(n1, 10);
        m.insert(n2, 20);

        assert_eq!(10, *m.get(&n1).unwrap());
        assert_eq!(20, *m.get(&n2).unwrap());
        assert_eq!(20, *m.get(&*n25).unwrap());
    }
}
