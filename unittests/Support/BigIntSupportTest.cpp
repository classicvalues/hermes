/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include <gtest/gtest.h>

#include "hermes/Support/BigIntSupport.h"
#include "hermes/Support/BigIntTestHelpers.h"

#include <vector>

namespace {

using namespace hermes;
using namespace hermes::bigint;

TEST(BigIntTest, numDigitsForSizeInBytesTest) {
  EXPECT_EQ(numDigitsForSizeInBytes(0), 0);

  for (size_t i = 0; i < 3; ++i) {
    for (size_t j = 1; j <= BigIntDigitSizeInBytes; ++j) {
      EXPECT_EQ(numDigitsForSizeInBytes(BigIntDigitSizeInBytes * i + j), i + 1)
          << BigIntDigitSizeInBytes * i + j;
    }
  }
}

TEST(BigIntTest, numDigitsForSizeInBitsTest) {
  EXPECT_EQ(numDigitsForSizeInBits(0), 0);

  for (size_t i = 0; i < 3; ++i) {
    for (size_t j = 1; j <= BigIntDigitSizeInBits; ++j) {
      EXPECT_EQ(numDigitsForSizeInBits(BigIntDigitSizeInBits * i + j), i + 1)
          << BigIntDigitSizeInBits * i + j;
    }
  }
}

TEST(BigIntTest, getSignExtValueTest) {
  // sanity-check some base values.
  static_assert(
      getSignExtValue<uint8_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<uint8_t>(0x80) == 0xff, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int8_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int8_t>(0x80) == -1, "Unexpected sign-ext value");

  static_assert(
      getSignExtValue<uint16_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<uint16_t>(0x80) == 0xffff, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int16_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int16_t>(0x80) == -1, "Unexpected sign-ext value");

  static_assert(
      getSignExtValue<uint32_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<uint32_t>(0x80) == 0xffffffff,
      "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int32_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int32_t>(0x80) == -1, "Unexpected sign-ext value");

  static_assert(
      getSignExtValue<uint64_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<uint64_t>(0x80) == 0xffffffffffffffffull,
      "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int64_t>(0x00) == 0, "Unexpected sign-ext value");
  static_assert(
      getSignExtValue<int64_t>(0x80) == -1, "Unexpected sign-ext value");

  for (uint32_t i = 0; i < 128; ++i) {
    EXPECT_EQ(getSignExtValue<uint8_t>(i), 0) << i;
    EXPECT_EQ(getSignExtValue<int8_t>(i), 0) << i;
    EXPECT_EQ(getSignExtValue<uint16_t>(i), 0) << i;
    EXPECT_EQ(getSignExtValue<int16_t>(i), 0) << i;
    EXPECT_EQ(getSignExtValue<uint32_t>(i), 0) << i;
    EXPECT_EQ(getSignExtValue<int32_t>(i), 0) << i;
    EXPECT_EQ(getSignExtValue<uint64_t>(i), 0) << i;
    EXPECT_EQ(getSignExtValue<int64_t>(i), 0) << i;
  }
  for (uint32_t i = 128; i < 256; ++i) {
    EXPECT_EQ(getSignExtValue<uint8_t>(i), 0xff) << i;
    EXPECT_EQ(getSignExtValue<int8_t>(i), -1) << i;
    EXPECT_EQ(getSignExtValue<uint16_t>(i), 0xffff) << i;
    EXPECT_EQ(getSignExtValue<int16_t>(i), -1) << i;
    EXPECT_EQ(getSignExtValue<uint32_t>(i), 0xffffffff) << i;
    EXPECT_EQ(getSignExtValue<int32_t>(i), -1) << i;
    EXPECT_EQ(getSignExtValue<uint64_t>(i), 0xffffffffffffffffull) << i;
    EXPECT_EQ(getSignExtValue<int64_t>(i), -1) << i;
  }
}

TEST(BigIntTest, dropExtraSignBitsTest) {
  // Special cases: empty sequence => empty sequence
  EXPECT_TRUE(dropExtraSignBits(llvh::ArrayRef<uint8_t>()).empty());

  // Special cases: sequence of zeros => empty sequence
  EXPECT_TRUE(dropExtraSignBits(llvh::makeArrayRef<uint8_t>({0})).empty());

  EXPECT_TRUE(dropExtraSignBits(llvh::makeArrayRef<uint8_t>({0, 0})).empty());

  EXPECT_TRUE(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>({0, 0, 0})).empty());

  EXPECT_TRUE(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>({0, 0, 0, 0})).empty());

  EXPECT_TRUE(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>({0, 0, 0, 0, 0})).empty());

  EXPECT_EQ(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>({0x7f})),
      llvh::makeArrayRef<uint8_t>({0x7f}));

  EXPECT_EQ(
      dropExtraSignBits(
          llvh::makeArrayRef<uint8_t>({0x7f, 0x00, 0x00, 0x00, 0x00})),
      llvh::makeArrayRef<uint8_t>({0x7f}));

  EXPECT_EQ(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>({0xff, 0xff, 0xff, 0xff})),
      llvh::makeArrayRef<uint8_t>({0xff}));

  EXPECT_EQ(
      dropExtraSignBits(
          llvh::makeArrayRef<uint8_t>({0xff, 0xff, 0xff, 0xff, 0xff})),
      llvh::makeArrayRef<uint8_t>({0xff}));

  EXPECT_EQ(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>(
          {0x00,
           0x01,
           0x02,
           0x03,
           0x03,
           0x00,
           0x00,
           0x00,
           0x02,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00})),
      llvh::makeArrayRef<uint8_t>(
          {0x00, 0x01, 0x02, 0x03, 0x03, 0x00, 0x00, 0x00, 0x02}));

  EXPECT_EQ(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>(
          {0x80,
           0x81,
           0x82,
           0x83,
           0x89,
           0x00,
           0x00,
           0x00,
           0x8a,
           0xff,
           0xff,
           0xff,
           0xff,
           0xff})),
      llvh::makeArrayRef<uint8_t>(
          {0x80, 0x81, 0x82, 0x83, 0x89, 0x00, 0x00, 0x00, 0x8a}));

  EXPECT_EQ(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>(
          {0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f})),
      llvh::makeArrayRef<uint8_t>({0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f}));

  EXPECT_EQ(
      dropExtraSignBits(llvh::makeArrayRef<uint8_t>(
          {0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x80})),
      llvh::makeArrayRef<uint8_t>(
          {0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x00,
           0x80}));
}

LeftToRightVector fillDigits(
    uint32_t numDigits,
    llvh::ArrayRef<uint8_t> bytes) {
  // initialize the resulting bytes with 0xdd helps spot bytes that are
  // uninitialized by initWithBytes.
  const BigIntDigitType kUninitialized = 0xddddddddddddddddull;

  // Always request at least 1 digit to avoid passing nullptr to
  // initWithBytes.
  std::vector<BigIntDigitType> result(std::max(1u, numDigits), kUninitialized);
  result.resize(numDigits);

  auto res = initWithBytes(MutableBigIntRef{result.data(), numDigits}, bytes);
  EXPECT_EQ(res, OperationStatus::RETURNED);

  // Create a byte view into result. Note that the number of digits in result is
  // **NOT** result.size(), but rather numDigits (which could have been modified
  // in initWithBytes).
  auto byteView = llvh::makeArrayRef(
      reinterpret_cast<const uint8_t *>(result.data()),
      numDigits * BigIntDigitSizeInBytes);

  // BigInt data is already LSB to MSB, so there's no LeftToRightVector
  // constructor that can be called. Thus create an empty LeftToRightVector and
  // populate its data member directly.
  LeftToRightVector ret;
  ret.data.insert(ret.data.end(), byteView.begin(), byteView.end());
  return ret;
}

TEST(BigIntTest, initWithBytesTest) {
  EXPECT_EQ(fillDigits(0, noDigits()), noDigits());

  EXPECT_EQ(fillDigits(1, noDigits()), noDigits());

  EXPECT_EQ(fillDigits(2, noDigits()), noDigits());

  EXPECT_EQ(fillDigits(1, digit(1, 2)), digit(0, 0, 0, 0, 0, 0, 1, 2));

  EXPECT_EQ(fillDigits(2, digit(1, 2)), digit(0, 0, 0, 0, 0, 0, 1, 2));

  EXPECT_EQ(
      fillDigits(2, digit(0) + digit(0x80, 0, 0, 0, 0, 0, 0, 0)),
      digit(0, 0, 0, 0, 0, 0, 0, 0) + digit(0x80, 0, 0, 0, 0, 0, 0, 0));

  EXPECT_EQ(
      fillDigits(2, digit(0xff) + digit(0x80, 0, 0, 0, 0, 0, 0, 0)),
      digit(0x80, 0, 0, 0, 0, 0, 0, 0));

  EXPECT_EQ(
      fillDigits(2, digit(0x80)),
      digit(0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x80));

  EXPECT_EQ(
      fillDigits(2, digit(1, 2, 3, 4, 5, 6, 7, 8)),
      digit(1, 2, 3, 4, 5, 6, 7, 8));

  EXPECT_EQ(
      fillDigits(2, digit(9) + digit(1, 2, 3, 4, 5, 6, 7, 8)),
      digit(0, 0, 0, 0, 0, 0, 0, 9) + digit(1, 2, 3, 4, 5, 6, 7, 8));
}

} // namespace
