#include "cu2qu.h"

#include <cmath>
#include <complex>
#include <cstdlib>
#include <limits>
#include <vector>

namespace {

using ComplexPoint = std::complex<double>;

constexpr int kMaxSplineSegments = 100;

struct CubicSegment {
  ComplexPoint p0;
  ComplexPoint p1;
  ComplexPoint p2;
  ComplexPoint p3;
};

ComplexPoint ToComplex(const cff_point_t &point) {
  return ComplexPoint(point.x, point.y);
}

cff_point_t ToPoint(const ComplexPoint &point) {
  cff_point_t out = {point.real(), point.imag()};
  return out;
}

bool IsFinitePoint(const cff_point_t &point) {
  return std::isfinite(point.x) && std::isfinite(point.y);
}

bool IsFiniteCurve(const cubic_curve_t *curve) {
  return IsFinitePoint(curve->p0) && IsFinitePoint(curve->p1) &&
         IsFinitePoint(curve->p2) && IsFinitePoint(curve->p3);
}

double dot(const ComplexPoint &v1, const ComplexPoint &v2) {
  double result = std::real(v1 * std::conj(v2));
  if (std::abs(result) < 1e-15) {
    result = 0.0;
  }
  return result;
}

ComplexPoint complex_div_by_real(const ComplexPoint &z, double den) {
  return ComplexPoint(z.real() / den, z.imag() / den);
}

CubicSegment calc_cubic_points(const ComplexPoint &a,
                               const ComplexPoint &b,
                               const ComplexPoint &c,
                               const ComplexPoint &d) {
  CubicSegment segment;
  segment.p0 = d;
  segment.p1 = complex_div_by_real(c, 3.0) + d;
  segment.p2 = complex_div_by_real(b + c, 3.0) + segment.p1;
  segment.p3 = a + d + c + b;
  return segment;
}

void calc_cubic_parameters(const ComplexPoint &p0,
                           const ComplexPoint &p1,
                           const ComplexPoint &p2,
                           const ComplexPoint &p3,
                           ComplexPoint *a,
                           ComplexPoint *b,
                           ComplexPoint *c,
                           ComplexPoint *d) {
  *c = (p1 - p0) * 3.0;
  *b = (p2 - p1) * 3.0 - *c;
  *d = p0;
  *a = p3 - *d - *c - *b;
}

std::vector<CubicSegment> split_cubic_into_two(const ComplexPoint &p0,
                                               const ComplexPoint &p1,
                                               const ComplexPoint &p2,
                                               const ComplexPoint &p3) {
  ComplexPoint mid = (p0 + 3.0 * (p1 + p2) + p3) * 0.125;
  ComplexPoint deriv3 = (p3 + p2 - p1 - p0) * 0.125;
  std::vector<CubicSegment> result;
  result.reserve(2);
  result.push_back({p0, (p0 + p1) * 0.5, mid - deriv3, mid});
  result.push_back({mid, mid + deriv3, (p2 + p3) * 0.5, p3});
  return result;
}

std::vector<CubicSegment> split_cubic_into_three(const ComplexPoint &p0,
                                                 const ComplexPoint &p1,
                                                 const ComplexPoint &p2,
                                                 const ComplexPoint &p3) {
  ComplexPoint mid1 = (8.0 * p0 + 12.0 * p1 + 6.0 * p2 + p3) * (1.0 / 27.0);
  ComplexPoint deriv1 = (p3 + 3.0 * p2 - 4.0 * p0) * (1.0 / 27.0);
  ComplexPoint mid2 = (p0 + 6.0 * p1 + 12.0 * p2 + 8.0 * p3) * (1.0 / 27.0);
  ComplexPoint deriv2 = (4.0 * p3 - 3.0 * p1 - p0) * (1.0 / 27.0);
  std::vector<CubicSegment> result;
  result.reserve(3);
  result.push_back({p0, (2.0 * p0 + p1) / 3.0, mid1 - deriv1, mid1});
  result.push_back({mid1, mid1 + deriv1, mid2 - deriv2, mid2});
  result.push_back({mid2, mid2 + deriv2, (p2 + 2.0 * p3) / 3.0, p3});
  return result;
}

std::vector<CubicSegment> split_cubic_into_n_iter(const ComplexPoint &p0,
                                                  const ComplexPoint &p1,
                                                  const ComplexPoint &p2,
                                                  const ComplexPoint &p3,
                                                  int n) {
  if (n == 2) {
    return split_cubic_into_two(p0, p1, p2, p3);
  }
  if (n == 3) {
    return split_cubic_into_three(p0, p1, p2, p3);
  }
  if (n == 4) {
    std::vector<CubicSegment> halves = split_cubic_into_two(p0, p1, p2, p3);
    std::vector<CubicSegment> result;
    result.reserve(4);
    for (const CubicSegment &half : halves) {
      std::vector<CubicSegment> quarter =
          split_cubic_into_two(half.p0, half.p1, half.p2, half.p3);
      result.insert(result.end(), quarter.begin(), quarter.end());
    }
    return result;
  }
  if (n == 6) {
    std::vector<CubicSegment> halves = split_cubic_into_two(p0, p1, p2, p3);
    std::vector<CubicSegment> result;
    result.reserve(6);
    for (const CubicSegment &half : halves) {
      std::vector<CubicSegment> third =
          split_cubic_into_three(half.p0, half.p1, half.p2, half.p3);
      result.insert(result.end(), third.begin(), third.end());
    }
    return result;
  }

  ComplexPoint a;
  ComplexPoint b;
  ComplexPoint c;
  ComplexPoint d;
  calc_cubic_parameters(p0, p1, p2, p3, &a, &b, &c, &d);

  double dt = 1.0 / static_cast<double>(n);
  double delta_2 = dt * dt;
  double delta_3 = dt * delta_2;

  std::vector<CubicSegment> result;
  result.reserve(static_cast<size_t>(n));
  for (int i = 0; i < n; ++i) {
    double t1 = static_cast<double>(i) * dt;
    double t1_2 = t1 * t1;
    ComplexPoint a1 = a * delta_3;
    ComplexPoint b1 = (3.0 * a * t1 + b) * delta_2;
    ComplexPoint c1 = (2.0 * b * t1 + c + 3.0 * a * t1_2) * dt;
    ComplexPoint d1 = a * t1 * t1_2 + b * t1_2 + c * t1 + d;
    result.push_back(calc_cubic_points(a1, b1, c1, d1));
  }

  return result;
}

ComplexPoint cubic_approx_control(double t,
                                  const ComplexPoint &p0,
                                  const ComplexPoint &p1,
                                  const ComplexPoint &p2,
                                  const ComplexPoint &p3) {
  ComplexPoint q1 = p0 + (p1 - p0) * 1.5;
  ComplexPoint q2 = p3 + (p2 - p3) * 1.5;
  return q1 + (q2 - q1) * t;
}

ComplexPoint calc_intersect(const ComplexPoint &a,
                            const ComplexPoint &b,
                            const ComplexPoint &c,
                            const ComplexPoint &d) {
  ComplexPoint ab = b - a;
  ComplexPoint cd = d - c;
  ComplexPoint p = ab * ComplexPoint(0.0, 1.0);
  double denominator = dot(p, cd);
  if (denominator == 0.0) {
    if (b == c && (a == b || c == d)) {
      return b;
    }
    double nan = std::numeric_limits<double>::quiet_NaN();
    return ComplexPoint(nan, nan);
  }

  double h = dot(p, a - c) / denominator;
  return c + cd * h;
}

bool cubic_farthest_fit_inside(const ComplexPoint &p0,
                               const ComplexPoint &p1,
                               const ComplexPoint &p2,
                               const ComplexPoint &p3,
                               double tolerance) {
  if (std::abs(p2) <= tolerance && std::abs(p1) <= tolerance) {
    return true;
  }

  ComplexPoint mid = (p0 + 3.0 * (p1 + p2) + p3) * 0.125;
  if (std::abs(mid) > tolerance) {
    return false;
  }

  ComplexPoint deriv3 = (p3 + p2 - p1 - p0) * 0.125;
  return cubic_farthest_fit_inside(p0, (p0 + p1) * 0.5, mid - deriv3, mid,
                                   tolerance) &&
         cubic_farthest_fit_inside(mid, mid + deriv3, (p2 + p3) * 0.5, p3,
                                   tolerance);
}

bool cubic_approx_quadratic(const CubicSegment &cubic,
                            double tolerance,
                            std::vector<ComplexPoint> *out) {
  ComplexPoint q1 = calc_intersect(cubic.p0, cubic.p1, cubic.p2, cubic.p3);
  if (std::isnan(q1.imag())) {
    return false;
  }

  ComplexPoint c1 = cubic.p0 + (q1 - cubic.p0) * (2.0 / 3.0);
  ComplexPoint c2 = cubic.p3 + (q1 - cubic.p3) * (2.0 / 3.0);
  if (!cubic_farthest_fit_inside(ComplexPoint(0.0, 0.0), c1 - cubic.p1,
                                 c2 - cubic.p2, ComplexPoint(0.0, 0.0),
                                 tolerance)) {
    return false;
  }

  out->clear();
  out->reserve(3);
  out->push_back(cubic.p0);
  out->push_back(q1);
  out->push_back(cubic.p3);
  return true;
}

bool cubic_approx_spline(const CubicSegment &cubic,
                         int n,
                         double tolerance,
                         bool all_quadratic,
                         std::vector<ComplexPoint> *out) {
  if (n == 1) {
    return cubic_approx_quadratic(cubic, tolerance, out);
  }
  if (n == 2 && !all_quadratic) {
    out->clear();
    out->reserve(4);
    out->push_back(cubic.p0);
    out->push_back(cubic.p1);
    out->push_back(cubic.p2);
    out->push_back(cubic.p3);
    return true;
  }

  std::vector<CubicSegment> cubics =
      split_cubic_into_n_iter(cubic.p0, cubic.p1, cubic.p2, cubic.p3, n);

  CubicSegment next_cubic = cubics[0];
  ComplexPoint next_q1 =
      cubic_approx_control(0.0, next_cubic.p0, next_cubic.p1, next_cubic.p2,
                           next_cubic.p3);
  ComplexPoint q2 = cubic.p0;
  ComplexPoint d1(0.0, 0.0);

  out->clear();
  out->reserve(static_cast<size_t>(n) + 2);
  out->push_back(cubic.p0);
  out->push_back(next_q1);

  for (int i = 1; i <= n; ++i) {
    ComplexPoint c1 = next_cubic.p1;
    ComplexPoint c2 = next_cubic.p2;
    ComplexPoint c3 = next_cubic.p3;

    ComplexPoint q0 = q2;
    ComplexPoint q1 = next_q1;
    if (i < n) {
      next_cubic = cubics[static_cast<size_t>(i)];
      next_q1 = cubic_approx_control(static_cast<double>(i) / (n - 1),
                                     next_cubic.p0, next_cubic.p1,
                                     next_cubic.p2, next_cubic.p3);
      out->push_back(next_q1);
      q2 = (q1 + next_q1) * 0.5;
    } else {
      q2 = c3;
    }

    ComplexPoint d0 = d1;
    d1 = q2 - c3;

    if (std::abs(d1) > tolerance ||
        !cubic_farthest_fit_inside(
            d0, q0 + (q1 - q0) * (2.0 / 3.0) - c1,
            q2 + (q1 - q2) * (2.0 / 3.0) - c2, d1, tolerance)) {
      out->clear();
      return false;
    }
  }

  out->push_back(cubic.p3);
  return true;
}

}  // namespace

extern "C" void quadratic_spline_destroy(quadratic_spline_t *spline) {
  if (spline == nullptr) {
    return;
  }

  std::free(spline->points);
  spline->points = nullptr;
  spline->num_points = 0;
  spline->closed = 0;
}

extern "C" eot_status_t curve_to_quadratic(const cubic_curve_t *curve,
                                           double max_err,
                                           quadratic_spline_t *out_spline) {
  if (curve == nullptr || out_spline == nullptr) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  quadratic_spline_destroy(out_spline);

  if (!(max_err > 0.0) || !std::isfinite(max_err) || !IsFiniteCurve(curve)) {
    return EOT_ERR_INVALID_ARGUMENT;
  }

  CubicSegment cubic = {ToComplex(curve->p0), ToComplex(curve->p1),
                        ToComplex(curve->p2), ToComplex(curve->p3)};

  for (int n = 1; n <= kMaxSplineSegments; ++n) {
    std::vector<ComplexPoint> spline_points;
    if (!cubic_approx_spline(cubic, n, max_err, true, &spline_points)) {
      continue;
    }

    cff_point_t *points = static_cast<cff_point_t *>(
        std::malloc(spline_points.size() * sizeof(cff_point_t)));
    if (points == nullptr) {
      return EOT_ERR_ALLOCATION;
    }

    for (size_t i = 0; i < spline_points.size(); ++i) {
      points[i] = ToPoint(spline_points[i]);
    }

    out_spline->points = points;
    out_spline->num_points = spline_points.size();
    out_spline->closed = 0;
    return EOT_OK;
  }

  /* No better status exists yet for exhausting the segment search cap. */
  return EOT_ERR_CORRUPT_DATA;
}
