"""
Common, generic metrics utilities for use across benchmarks & evals

TODO: move into `qt` CLI for use across languages.
"""

import math
from collections.abc import Iterable
from typing import SupportsFloat, final

import numpy as np
import scipy.stats


@final
class Statistics:
  """
  A namespace that groups metrics for computing statistics of discrete
  metrics
  """

  @staticmethod
  def accuracy(num_correct: int, total_num: int) -> float:
    """
    Return the ratio of correct values to total values. Most often used
    for QA-style benchmarks, where the model needs to determine the correct
    answer from a discrete list of possible answers.
    """
    return float(num_correct) / float(total_num) if total_num > 0 else 0.0

  @staticmethod
  def mean(vals: Iterable[SupportsFloat]) -> float:
    """Get the standard average of all values in ``vals``"""
    np_vals = np.fromiter(vals, dtype=np.float64)
    if np_vals.size == 0:
      raise ValueError("vals cannot be empty")
    return float(np.mean(np_vals))

  @staticmethod
  def clipped_mean(
    vals: Iterable[SupportsFloat], min_max: tuple[float, float] | None = None
  ) -> float:
    """
    Return the average of all values in ``vals``, where each value
    is first constrained ("clipped") to a fixed range before computing the mean.

    When clipping to [0, 1], any value below 0 becomes 0, and any value above 1 becomes 1.
    This is useful when you want aggregate metrics to remain stable and interpretable even in the
    presence of outliers, noisy scores, or malformed values.
    """
    np_vals = np.fromiter(vals, dtype=np.float64)
    if np_vals.size == 0:
      raise ValueError("vals cannot be empty")
    min = min_max[0] if min_max else 0.0
    max = min_max[1] if min_max else 1.0
    if min >= max:
      raise ValueError(f"min cannot be greater than or equal to max ([min, max]=[{min}, {max}])")
    return float(np.clip(np_vals, min, max).mean())

  @staticmethod
  def variance(
    vals: Iterable[SupportsFloat],
    *,
    sample: bool = False,
  ) -> float:
    """
    Compute the variance of the values in ``vals``.

    Variance measures how spread out values are around the mean by
    averaging the squared distance from the mean. It is commonly used
    to quantify consistency, volatility, or uncertainty in a dataset.
    """
    np_vals = np.fromiter(
      (float(v) for v in vals),
      dtype=np.float64,
    )

    if np_vals.size == 0:
      raise ValueError("vals cannot be empty")

    ddof = 1 if sample else 0
    return float(np.var(np_vals, ddof=ddof))

  @staticmethod
  def stddev(
    vals: Iterable[SupportsFloat],
    *,
    sample: bool = False,
  ) -> float:
    """
    Compute the standard deviation of the values in ``vals``.

    Standard deviation is the square root of the variance and measures
    how dispersed values are around the mean using the same units as the
    original data. It is often easier to interpret than variance because
    it is expressed on the same scale as the input values.
    """
    return math.sqrt(Statistics.variance(vals, sample=sample))

  @staticmethod
  def confidence_interval(
    vals: Iterable[SupportsFloat],
    *,
    confidence: float = 0.95,
  ) -> tuple[float, float]:
    """
    Compute a confidence interval for the population mean.

    Uses the Student's t-distribution, which is exact when the underlying
    population is normal and a good approximation for moderate sample sizes.
    The t-distribution automatically converges to the normal (z) distribution
    as the sample size grows, so it is safe to use for any ``n >= 2``.

    A confidence interval estimates a range that is likely to contain the
    true population mean given the observed sample data. It is useful for
    quantifying uncertainty in aggregate metrics and understanding how
    stable or statistically reliable an evaluation result is.
    """

    np_vals = np.fromiter((float(v) for v in vals), dtype=np.float64)

    n = np_vals.size
    if n == 0:
      raise ValueError("vals cannot be empty")

    mean = float(np.mean(np_vals))
    if n == 1:
      return (mean, mean)

    std = float(np.std(np_vals, ddof=1))
    alpha = 1.0 - confidence
    t_crit = scipy.stats.t.ppf(1.0 - alpha / 2.0, df=n - 1)
    margin = t_crit * (std / np.sqrt(n))
    return (mean - margin, mean + margin)


@final
class Classification:
  """
  A namespace that groups metrics for measuring binary classifiers.
  """

  @staticmethod
  def precision(
    y_true: Iterable[bool],
    y_pred: Iterable[bool],
  ) -> float:
    """
    Compute the precision of a binary classifier.

    Precision measures how often positive predictions are correct. It is
    useful when false positives are costly, such as in spam detection,
    hallucination detection, or medical screening systems.
    """

    true = np.asarray(list(y_true), dtype=bool)
    pred = np.asarray(list(y_pred), dtype=bool)

    tp = np.sum(true & pred)
    fp = np.sum(~true & pred)
    denom = tp + fp
    return 0.0 if denom == 0 else float(tp / denom)

  @staticmethod
  def recall(
    y_true: Iterable[bool],
    y_pred: Iterable[bool],
  ) -> float:
    """
    Compute the recall of a binary classifier.

    Recall measures how many actual positive cases were correctly
    identified by the model. It is useful when false negatives are costly,
    such as in disease detection or safety-critical monitoring systems.
    """

    true = np.asarray(list(y_true), dtype=bool)
    pred = np.asarray(list(y_pred), dtype=bool)
    tp = np.sum(true & pred)
    fn = np.sum(true & ~pred)
    denom = tp + fn
    return 0.0 if denom == 0 else float(tp / denom)

  @staticmethod
  def specificity(
    y_true: Iterable[bool],
    y_pred: Iterable[bool],
  ) -> float:
    """
    Compute the specificity of a binary classifier.

    Specificity measures how many actual negative cases were correctly
    identified by the model. It is useful for understanding how well a
    system avoids false alarms or incorrect positive predictions.
    """

    true = np.asarray(list(y_true), dtype=bool)
    pred = np.asarray(list(y_pred), dtype=bool)

    tn = np.sum(~true & ~pred)
    fp = np.sum(~true & pred)
    denom = tn + fp
    return 0.0 if denom == 0 else float(tn / denom)

  @staticmethod
  def f1(
    y_true: Iterable[bool],
    y_pred: Iterable[bool],
  ) -> float:
    """
    Compute the F1 score of a binary classifier.

    The F1 score is the harmonic mean of precision and recall and provides
    a balanced measure of classifier performance. It is commonly used when
    both false positives and false negatives are important.
    """

    p = Classification.precision(y_true, y_pred)
    r = Classification.recall(y_true, y_pred)
    denom = p + r
    return 0.0 if denom == 0 else float(2.0 * (p * r) / denom)
