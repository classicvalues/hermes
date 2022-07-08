/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow strict-local
 * @format
 */

import {t, transform} from '../test_codemods/test-utils';

describe('function parameter spacing', () => {
  function codemod(code: string) {
    return transform(code, context => ({
      FunctionExpression(node) {
        context.replaceNode(
          node,
          t.ArrowFunctionExpression({
            async: node.async,
            body: context.shallowCloneNode(node.body),
            params: context.shallowCloneArray(node.params),
            predicate: context.shallowCloneNode(node.predicate),
            returnType: context.shallowCloneNode(node.returnType),
            typeParameters: context.shallowCloneNode(node.typeParameters),
          }),
        );
      },
    }));
  }

  it('should not add newlines between params that fit on one line', () => {
    const result = codemod(`\
/**
 */

const foo = function foo(a, b, c, d, e, f) {
  // body


  // the above double empty line should be collapsed
};
`);
    expect(result).toBe(`\
/**
 */

const foo = (a, b, c, d, e, f) => {
  // body
  // the above double empty line should be collapsed
};
`);
  });

  it("should not add newlines between params that don't fit on one line", () => {
    const result = codemod(`\
/**
 */

const foo = function foo(a, b, c, d, e, f, a_really_long_param, that_forces_this_onto_a_new_line) {
  // body


  // the above double empty line should be collapsed
};
`);
    expect(result).toBe(`\
/**
 */

const foo = (
  a,
  b,
  c,
  d,
  e,
  f,
  a_really_long_param,
  that_forces_this_onto_a_new_line,
) => {
  // body
  // the above double empty line should be collapsed
};
`);
  });
});

describe('call argument spacing', () => {
  function codemod(code: string) {
    return transform(code, context => ({
      NewExpression(node) {
        context.replaceNode(
          node,
          t.CallExpression({
            callee: context.shallowCloneNode(node.callee),
            arguments: context.shallowCloneArray(node.arguments),
          }),
        );
      },
    }));
  }

  it('should not add newlines between arguments that fit on one line', () => {
    const result = codemod(`\
/**
 */

throw new Foo(
  a,
  b,
  c,
  d,
  e,
  f,
);
`);
    expect(result).toBe(`\
/**
 */

throw Foo(a, b, c, d, e, f);
`);
  });

  it("should not add newlines between arguments that don't fit on one line", () => {
    const result = codemod(`\
/**
 */

throw new Foo(a, b, c, d, e, f, a_really_long_param, that_forces_this_onto_a_new_line);
`);
    expect(result).toBe(`\
/**
 */

throw Foo(
  a,
  b,
  c,
  d,
  e,
  f,
  a_really_long_param,
  that_forces_this_onto_a_new_line,
);
`);
  });
});

describe('call with function spacing', () => {
  function codemod(code: string) {
    return transform(code, context => ({
      CallExpression(node) {
        if (
          node.arguments.length === 0 ||
          node.arguments[0].type !== 'FunctionExpression'
        ) {
          return;
        }
        const callback = node.arguments[0];
        context.replaceNode(
          node,
          context.shallowCloneNodeWithOverrides(node, {
            arguments: [
              t.ArrowFunctionExpression({
                async: callback.async,
                body: context.shallowCloneNode(callback.body),
                params: context.shallowCloneArray(callback.params),
                predicate: context.shallowCloneNode(callback.predicate),
                returnType: context.shallowCloneNode(callback.returnType),
                typeParameters: context.shallowCloneNode(
                  callback.typeParameters,
                ),
              }),
            ],
          }),
        );
      },
    }));
  }

  it('should not add newlines between params that fit on one line within a call expression', () => {
    const result = codemod(`\
/**
 */

const foo = [].forEach(function foo(a, b, c) {
  // body


  // the above double empty line should be collapsed
});
`);
    expect(result).toBe(`\
/**
 */

const foo = [].forEach((a, b, c) => {
  // body
  // the above double empty line should be collapsed
});
`);
  });

  it("should not add newlines between params that don't fit on one line within a call expression", () => {
    const result = codemod(`\
/**
 */

const foo = [].forEach(function foo(a, b, long, enough, to, _break, out, of, the, call, but, not, the, _function) {
  // body


  // the above double empty line should be collapsed
});
`);
    expect(result).toBe(`\
/**
 */

const foo = [].forEach(
  (
    a,
    b,
    long,
    enough,
    to,
    _break,
    out,
    of,
    the,
    call,
    but,
    not,
    the,
    _function,
  ) => {
    // body
    // the above double empty line should be collapsed
  },
);
`);
  });
});

describe('class member spacing', () => {
  function codemod(code: string) {
    return transform(code, context => ({
      ClassDeclaration(node) {
        context.replaceNode(
          node,
          t.ClassDeclaration({
            body: context.shallowCloneNode(node.body),
            id: t.Identifier({name: `${node.id?.name ?? 'Class'}_Replaced`}),
          }),
        );
      },
    }));
  }

  it('should not add or remove spaces between class members', () => {
    const result = codemod(`\
/**
 */

class Foo {
  a() {}
  b = 1;
  c = 2;
  replaceMe() {
    // body here
  }
}
class Bar {
  a() {}

  b = 1;

  c = 2;

  replaceMe() {
    // body here
  }
}
class Baz {
  a() {}

  b = 1;
  c = 2;

  replaceMe() {
    // body here
  }
}
`);
    expect(result).toBe(`\
/**
 */

class Foo_Replaced {
  a() {}
  b = 1;
  c = 2;
  replaceMe() {
    // body here
  }
}
class Bar_Replaced {
  a() {}

  b = 1;

  c = 2;

  replaceMe() {
    // body here
  }
}
class Baz_Replaced {
  a() {}

  b = 1;
  c = 2;

  replaceMe() {
    // body here
  }
}
`);
  });
});
