# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

from __future__ import annotations

import numpy as np
from shape_extensions import assert_raises, assert_shape


# https://numpy.org/doc/stable/reference/generated/numpy.ndarray.flags.html
# WRITEABLE is mutable through its lowercase attribute or W/WRITEABLE keys.
def test_writeable_flag_attribute() -> None:
    array = np.ones((2, 3))

    assert array.flags.writeable  # E: has no attribute `writeable`
    array.flags.writeable = False  # E: has no attribute `writeable`
    assert not array.flags["W"]
    with assert_raises(ValueError):
        array[0, 0] = 2.0
    array.flags.writeable = True  # E: has no attribute `writeable`
    array[0, 0] = 2.0
    assert array[0, 0] == 2.0
    assert_shape(array.shape, (2, 3))


def test_writeable_flag_dictionary_assignment() -> None:
    array = np.ones(())

    array.flags["W"] = False  # E: has no attribute `__setitem__`
    assert not array.flags["WRITEABLE"]
    with assert_raises(ValueError):
        array[()] = 2.0
    array.flags["WRITEABLE"] = True  # E: has no attribute `__setitem__`
    array[()] = 2.0
    assert array[()] == 2.0
    assert_shape(array.shape, ())
