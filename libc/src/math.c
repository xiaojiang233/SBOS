/*
 * Floating point helpers for the SBOS C library.
 *
 * Only the functions the user programs actually need are implemented, and they
 * are implemented for real rather than stubbed: pow() is used by the shell
 * ports (Bash's string-to-double conversion) and by anything that does its own
 * numeric parsing.
 */
#include <errno.h>
#include <math.h>

#define DOUBLE_SIGN      0x8000000000000000ULL
#define DOUBLE_EXPONENT  0x7ff0000000000000ULL
#define DOUBLE_MANTISSA  0x000fffffffffffffULL

typedef union {
    double value;
    unsigned long long bits;
} double_parts;

/* 1 / ln(2), used to turn a natural logarithm scale into a binary one. */
static const double inverse_ln_two = 1.4426950408889634074;
/* ln(2), used to turn a binary scale back into a natural one. */
static const double ln_two = 0.69314718055994530942;

static int is_not_a_number(double value) {
    double_parts parts;
    parts.value = value;
    return (parts.bits & DOUBLE_EXPONENT) == DOUBLE_EXPONENT
        && (parts.bits & DOUBLE_MANTISSA) != 0;
}

/* True for whole numbers: the value must equal its own truncation. */
static int is_whole(double value) {
    double truncated = (double)(long long)value;
    return truncated == value;
}

/* 2 raised to an integer power, exact while the result stays representable. */
static double two_to_the(int exponent) {
    double result = 1.0;
    while (exponent > 0) {
        result *= 2.0;
        exponent -= 1;
    }
    while (exponent < 0) {
        result *= 0.5;
        exponent += 1;
    }
    return result;
}

/*
 * log2() for a strictly positive value. The value is split into a mantissa in
 * [1, 2) and an exponent by reading the IEEE field, then the mantissa's
 * logarithm comes from the atanh series
 *
 *   log2(m) = (2 / ln 2) * (t + t^3/3 + t^5/5 + ...),  t = (m - 1) / (m + 1)
 *
 * With m in [1, 2) the parameter t never exceeds 1/3, so twenty terms are far
 * more than enough for a double.
 */
static double log2_of(double value) {
    double_parts parts;
    int exponent;
    double mantissa;
    double t;
    double t_squared;
    double term;
    double sum;
    int index;

    parts.value = value;
    exponent = (int)((parts.bits >> 52) & 0x7ff) - 1023;
    parts.bits = (parts.bits & DOUBLE_MANTISSA) | 0x3ff0000000000000ULL;
    mantissa = parts.value;

    t = (mantissa - 1.0) / (mantissa + 1.0);
    t_squared = t * t;
    term = t;
    sum = 0.0;
    for (index = 1; index <= 41; index += 2) {
        sum += term / (double)index;
        term *= t_squared;
    }
    return (double)exponent + sum * 2.0 * inverse_ln_two;
}

/* 2 raised to a fractional power, via the series 2^f = e^(f ln 2). */
static double two_to_the_fraction(double fraction) {
    double scaled = fraction * ln_two;
    double term = 1.0;
    double sum = 1.0;
    int index;

    /* e^scaled by its Taylor series; scaled stays within (-0.35, 0.35). */
    for (index = 1; index <= 18; ++index) {
        term *= scaled / (double)index;
        sum += term;
    }
    return sum;
}

static double exp2_of(double exponent) {
    double whole = (double)(long long)exponent;
    if (whole > exponent) {
        whole -= 1.0;
    } else if (whole < exponent - 1.0) {
        whole += 1.0;
    }
    return two_to_the((int)whole) * two_to_the_fraction(exponent - whole);
}

double pow(double base, double exponent) {
    double magnitude;
    double power;
    double result;

    if (exponent == 0.0) {
        return 1.0;
    }
    if (base == 1.0) {
        return 1.0;
    }
    if (is_not_a_number(base) || is_not_a_number(exponent)) {
        return base + exponent;
    }
    if (base == 0.0) {
        if (exponent < 0.0) {
            errno = ERANGE;
            return HUGE_VAL;
        }
        return 0.0;
    }

    /* Whole exponents away from zero take exact repeated squaring: this keeps
       powers of ten exact, which is what numeric parsing depends on. */
    if (is_whole(exponent) && exponent > -100000.0 && exponent < 100000.0) {
        long long remaining = (long long)exponent;
        int negative = remaining < 0;
        int odd = 0;
        result = 1.0;
        magnitude = base < 0.0 ? -base : base;
        if (negative) {
            remaining = -remaining;
        }
        if (remaining & 1) {
            odd = 1;
        }
        while (remaining > 0) {
            if (remaining & 1) {
                result *= magnitude;
            }
            remaining >>= 1;
            if (remaining != 0) {
                magnitude *= magnitude;
            }
        }
        if (base < 0.0 && odd) {
            result = -result;
        }
        if (negative) {
            if (result == 0.0) {
                errno = ERANGE;
                return HUGE_VAL;
            }
            result = 1.0 / result;
        }
        return result;
    }

    if (base < 0.0) {
        /* A negative base with a fractional exponent has no real result. */
        errno = EDOM;
        return __builtin_nan("");
    }

    power = exp2_of(exponent * log2_of(base));
    if (!is_whole(power) && power > 1.0e308) {
        errno = ERANGE;
        return HUGE_VAL;
    }
    return power;
}
