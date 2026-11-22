/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::testcase;

testcase!(
    bug = "Unhashable set elements are not reported",
    test_unhashable_set_element,
    r#"
def f(xs: list[int], d: dict[str, int]):
    {[1]}
    {1, xs}
    {d}
    {x for x in [xs]}
    {*[xs]}  # Elements of an unpacked iterable are not checked.
    {[]}  # Types that contain unsolved type variables are not checked.
"#,
);

testcase!(
    bug = "Unhashable dict keys are not reported",
    test_unhashable_dict_key,
    r#"
def f(xs: list[int], s: set[int]):
    {[1]: 1}
    {"a": 1, s: 2}
    {k: 0 for k in [xs]}
"#,
);

testcase!(
    bug = "Unhashable set elements and dict keys are not reported",
    test_unhashable_with_hint,
    r#"
x: set[list[int]] = {[1]}
y: dict[list[int], int] = {[1]: 1}
z: set[object] = {[1]}
w: dict[object, int] = {[1]: 1}
"#,
);

testcase!(
    test_hashable_keys,
    r#"
from typing import Any, Hashable, Literal, TypeVar

T = TypeVar("T")

class C:
    pass

def f(
    a: int,
    b: str,
    c: tuple[int, str],
    d: frozenset[int],
    e: None,
    g: C,
    h: Any,
    i: object,
    j: T,
    k: Literal["a"],
    l: Hashable,
    m: type[list[int]],
) -> T:
    {a, b, c, d, e, g, h, i, j, k, l, m}
    {a: 0, b: 0, c: 0, d: 0, e: 0, g: 0, h: 0, i: 0, j: 0, k: 0, l: 0, m: 0}
    return j
"#,
);

testcase!(
    bug = "Unhashable unions, bounded type variables, and classes are not reported",
    test_unhashable_types,
    r#"
from dataclasses import dataclass
from typing import ClassVar, TypeVar

T = TypeVar("T", bound=list[int])

class NoHash:
    __hash__: ClassVar[None]  # type: ignore[assignment]

@dataclass
class Eq:
    x: int

@dataclass(frozen=True)
class Frozen:
    x: int

def f(u: int | list[int], t: T, n: NoHash, e: Eq, fr: Frozen, tup: tuple[list[int]]) -> T:
    {u}
    {t}
    {n}
    {e}
    {fr}
    {tup}  # The tuple's elements are not checked.
    return t
"#,
);
