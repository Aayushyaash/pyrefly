/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::test::util::shape_extensions_env;
use crate::testcase;

// https://github.com/pyrefly-org/pyrefly/issues/3656
testcase!(
    bug = "A type variable that appears only once in a signature relates no types",
    test_type_var_used_once,
    r#"
from typing import TypeVar

T = TypeVar("T")

def pep695[S](x: S) -> None: ...
def legacy(x: T) -> None: ...
def bounded[S: int](x: S) -> None: ...
def union_member[S](x: S | None) -> None: ...
def return_only[S]() -> S: ...

class A:
    def method(self, x: T) -> None: ...
"#,
);

testcase!(
    test_type_var_used_once_exemptions,
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
    shape_extensions_env(),
    r#"
from shape_extensions import IntTuple, IntVar

class Tensor[Shape: IntTuple]:
    shape: Shape

def rank_one[BatchSize: IntVar](x: Tensor[[BatchSize]]) -> None: ...
def make_rank_one[BatchSize: IntVar]() -> Tensor[[BatchSize]]: ...
"#,
);
