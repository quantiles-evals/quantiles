import hashlib
import json

from quantiles.util import error_message, hash_json, stable_stringify


class TestStableStringify:
  def test_null(self) -> None:
    assert stable_stringify(None) == "null"

  def test_bool(self) -> None:
    assert stable_stringify(True) == "true"
    assert stable_stringify(False) == "false"

  def test_number(self) -> None:
    assert stable_stringify(42) == "42"
    assert stable_stringify(3.14) == "3.14"

  def test_string(self) -> None:
    assert stable_stringify("hello") == '"hello"'

  def test_empty_list(self) -> None:
    assert stable_stringify([]) == "[]"

  def test_list(self) -> None:
    assert stable_stringify([1, 2, 3]) == "[1,2,3]"

  def test_nested_list(self) -> None:
    assert stable_stringify([[1, 2], [3, 4]]) == "[[1,2],[3,4]]"

  def test_empty_object(self) -> None:
    assert stable_stringify({}) == "{}"

  def test_object_sorted_keys(self) -> None:
    assert stable_stringify({"b": 1, "a": 2}) == '{"a":2,"b":1}'

  def test_object_nested(self) -> None:
    assert stable_stringify({"b": {"d": 1, "c": 2}, "a": 3}) == '{"a":3,"b":{"c":2,"d":1}}'

  def test_object_filters_none(self) -> None:
    assert stable_stringify({"a": 1, "b": None, "c": 2}) == '{"a":1,"c":2}'

  def test_complex(self) -> None:
    value = json.loads('{"z": [null, true, {"y": 2, "x": 1}], "a": {"nested": [3, 2, 1]}}')
    expected = '{"a":{"nested":[3,2,1]},"z":[null,true,{"x":1,"y":2}]}'
    assert stable_stringify(value) == expected

  def test_deterministic(self) -> None:
    a = json.loads('{"z": 1, "a": 2, "m": {"c": 1, "b": 2}}')
    b = json.loads('{"a": 2, "m": {"b": 2, "c": 1}, "z": 1}')
    assert stable_stringify(a) == stable_stringify(b)


class TestHashJson:
  def test_hash_consistency(self) -> None:
    a = json.loads('{"z": 1, "a": 2}')
    b = json.loads('{"a": 2, "z": 1}')
    assert hash_json(a) == hash_json(b)

  def test_hash_differentiates(self) -> None:
    assert hash_json({"a": 1}) != hash_json({"a": 2})

  def test_hash_known_value(self) -> None:
    expected = hashlib.sha256(stable_stringify({"a": 1, "b": 2}).encode("utf-8")).hexdigest()
    assert hash_json({"b": 2, "a": 1}) == expected


class TestErrorMessage:
  def test_exception_includes_traceback(self) -> None:
    try:
      raise ValueError("test error")
    except ValueError as e:
      msg = error_message(e)
      assert "ValueError" in msg
      assert "test error" in msg
      assert "Traceback" in msg

  def test_non_exception(self) -> None:
    assert error_message("plain string") == "plain string"
    assert error_message(42) == "42"
