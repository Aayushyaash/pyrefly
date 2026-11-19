/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::test::util::TestEnv;
use crate::test::util::shape_extensions_env;
use crate::testcase;

// https://github.com/pyrefly-org/pyrefly/issues/3656
testcase!(
    test_type_var_used_once,
    TestEnv::new().enable_unnecessary_type_var_error(),
    r#"
from typing import TypeVar

T = TypeVar("T")

def pep695[S](x: S) -> None: ...  # E: Type variable `S` appears only once in the signature of `pep695`
def legacy(x: T) -> None: ...  # E: Type variable `T` appears only once in the signature of `legacy`
def bounded[S: int](x: S) -> None: ...  # E: Type variable `S` appears only once in the signature of `bounded`
def union_member[S](x: S | None) -> None: ...  # E: Type variable `S` appears only once in the signature of `union_member`
def return_only[S]() -> S: ...  # E: Type variable `S` appears only once in the signature of `return_only`

class A:
    def method(self, x: T) -> None: ...  # E: Type variable `T` appears only once in the signature of `method`
"#,
);

testcase!(
    test_type_var_used_once_exemptions,
    TestEnv::new().enable_unnecessary_type_var_error(),
    r#"
from typing import Callable, Generic, TypeVar

K = TypeVar("K")

def repeated[S](x: S) -> S: ...
def two_params[S](x: S, y: S) -> None: ...
def nested_param[S](x: list[S]) -> None: ...
def nested_return[S]() -> list[S]: ...
def constrained[S: (int, str)](x: S) -> None: ...
def with_default[S = int](x: S) -> None: ...
def param_spec[**P](f: Callable[P, None]) -> None: ...
def type_var_tuple[*Ts](*args: *Ts) -> None: ...
def inferred_return[S](x: S):
    return x

class Box(Generic[K]):
    def class_scoped(self, x: K) -> None: ...
    def implicit_self(self) -> None: ...
"#,
);

testcase!(
    test_int_var_used_once,
    shape_extensions_env().enable_unnecessary_type_var_error(),
    r#"
from shape_extensions import IntTuple, IntVar

class Tensor[Shape: IntTuple]:
    shape: Shape

def rank_one[BatchSize: IntVar](x: Tensor[[BatchSize]]) -> None: ...
def make_rank_one[BatchSize: IntVar]() -> Tensor[[BatchSize]]: ...
"#,
);
