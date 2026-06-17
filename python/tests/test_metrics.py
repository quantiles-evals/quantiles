"""Tests for quantiles.metrics module."""

import pytest

from quantiles.metrics import (
  Classification,
  Statistics,
)


def test_accuracy_perfect() -> None:
  assert Statistics.accuracy(5, 5) == 1.0


def test_accuracy_zero() -> None:
  assert Statistics.accuracy(0, 5) == 0.0


def test_accuracy_half() -> None:
  assert Statistics.accuracy(3, 6) == 0.5


def test_accuracy_empty() -> None:
  assert Statistics.accuracy(0, 0) == 0.0


def test_mean_basic() -> None:
  assert Statistics.mean([1.0, 2.0, 3.0]) == 2.0


def test_mean_ints() -> None:
  assert Statistics.mean([1, 2, 3]) == 2.0


def test_mean_single() -> None:
  assert Statistics.mean([42.0]) == 42.0


def test_mean_empty_raises() -> None:
  with pytest.raises(ValueError, match="cannot be empty"):
    Statistics.mean([])


def test_clipped_mean_default_range() -> None:
  vals = [-1.0, 0.5, 1.5, 0.0]
  assert Statistics.clipped_mean(vals) == 0.375


def test_clipped_mean_custom_range() -> None:
  vals = [0.0, 5.0, 10.0, 15.0]
  assert Statistics.clipped_mean(vals, min_max=(2.0, 8.0)) == pytest.approx(5.75)


def test_clipped_mean_empty_raises() -> None:
  with pytest.raises(ValueError, match="cannot be empty"):
    Statistics.clipped_mean([])


def test_clipped_mean_invalid_range_raises() -> None:
  with pytest.raises(ValueError, match="min cannot be greater than or equal to max"):
    Statistics.clipped_mean([1.0, 2.0], min_max=(5.0, 3.0))


def test_clipped_mean_equal_min_max_raises() -> None:
  with pytest.raises(ValueError, match="min cannot be greater than or equal to max"):
    Statistics.clipped_mean([1.0, 2.0], min_max=(3.0, 3.0))


def test_variance_population() -> None:
  vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
  assert Statistics.variance(vals) == pytest.approx(4.0)


def test_variance_sample() -> None:
  vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
  assert Statistics.variance(vals, sample=True) == pytest.approx(4.5714, rel=1e-3)


def test_variance_empty_raises() -> None:
  with pytest.raises(ValueError, match="cannot be empty"):
    Statistics.variance([])


def test_stddev_population() -> None:
  vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
  assert Statistics.stddev(vals) == pytest.approx(2.0)


def test_stddev_sample() -> None:
  vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
  assert Statistics.stddev(vals, sample=True) == pytest.approx(2.138, rel=1e-3)


def test_stddev_empty_raises() -> None:
  with pytest.raises(ValueError, match="cannot be empty"):
    Statistics.stddev([])


def test_confidence_interval_basic() -> None:
  vals = [1.0, 2.0, 3.0, 4.0, 5.0]
  low, high = Statistics.confidence_interval(vals)
  assert low < 3.0 < high


def test_confidence_interval_90_percent() -> None:
  vals = [1.0, 2.0, 3.0, 4.0, 5.0]
  low_90, high_90 = Statistics.confidence_interval(vals, confidence=0.90)
  low_95, high_95 = Statistics.confidence_interval(vals)
  assert low_90 > low_95
  assert high_90 < high_95


def test_confidence_interval_99_percent() -> None:
  vals = [1.0, 2.0, 3.0, 4.0, 5.0]
  low_99, high_99 = Statistics.confidence_interval(vals, confidence=0.99)
  low_95, high_95 = Statistics.confidence_interval(vals)
  assert low_99 < low_95
  assert high_99 > high_95


def test_confidence_interval_n_equals_one() -> None:
  vals = [42.0]
  low, high = Statistics.confidence_interval(vals)
  assert low == pytest.approx(42.0)
  assert high == pytest.approx(42.0)


def test_confidence_interval_empty_raises() -> None:
  with pytest.raises(ValueError, match="cannot be empty"):
    Statistics.confidence_interval([])


def test_confidence_interval_arbitrary_confidence() -> None:
  vals = [1.0, 2.0, 3.0, 4.0, 5.0]
  low_75, high_75 = Statistics.confidence_interval(vals, confidence=0.75)
  low_95, high_95 = Statistics.confidence_interval(vals)
  assert low_75 > low_95
  assert high_75 < high_95


def test_classification_precision_perfect() -> None:
  assert Classification.precision([True, True, False], [True, True, False]) == 1.0


def test_classification_precision_all_positive_predictions_wrong() -> None:
  assert Classification.precision([False, False, True], [True, True, False]) == 0.0


def test_classification_precision_no_positive_predictions() -> None:
  assert Classification.precision([True, True, False], [False, False, False]) == 0.0


def test_classification_precision_mixed() -> None:
  y_true = [True, True, False, False, True]
  y_pred = [True, False, True, False, True]
  tp = 2
  fp = 1
  assert Classification.precision(y_true, y_pred) == pytest.approx(tp / (tp + fp))


def test_classification_recall_perfect() -> None:
  assert Classification.recall([True, True, False], [True, True, False]) == 1.0


def test_classification_recall_all_positive_predictions_wrong() -> None:
  assert Classification.recall([True, True, False], [False, False, False]) == 0.0


def test_classification_recall_no_true_positives() -> None:
  assert Classification.recall([False, False, False], [True, True, True]) == 0.0


def test_classification_recall_mixed() -> None:
  y_true = [True, True, False, False, True]
  y_pred = [True, False, True, False, True]
  tp = 2
  fn = 1
  assert Classification.recall(y_true, y_pred) == pytest.approx(tp / (tp + fn))


def test_classification_specificity_perfect() -> None:
  assert Classification.specificity([True, True, False], [True, True, False]) == 1.0


def test_classification_specificity_all_negative_predictions_wrong() -> None:
  assert Classification.specificity([False, False, True], [True, True, True]) == 0.0


def test_classification_specificity_no_true_negatives() -> None:
  assert Classification.specificity([True, True, True], [False, False, False]) == 0.0


def test_classification_specificity_mixed() -> None:
  y_true = [True, False, False, True, False]
  y_pred = [False, True, False, True, False]
  tn = 2
  fp = 1
  assert Classification.specificity(y_true, y_pred) == pytest.approx(tn / (tn + fp))


def test_classification_f1_perfect() -> None:
  assert Classification.f1([True, True, False], [True, True, False]) == 1.0


def test_classification_f1_zero_precision() -> None:
  assert Classification.f1([False, False, True], [True, True, False]) == 0.0


def test_classification_f1_zero_recall() -> None:
  assert Classification.f1([True, True, False], [False, False, False]) == 0.0


def test_classification_f1_mixed() -> None:
  y_true = [True, True, False, False, True]
  y_pred = [True, False, True, False, True]
  p = Classification.precision(y_true, y_pred)
  r = Classification.recall(y_true, y_pred)
  expected = 2 * p * r / (p + r)
  assert Classification.f1(y_true, y_pred) == pytest.approx(expected)
