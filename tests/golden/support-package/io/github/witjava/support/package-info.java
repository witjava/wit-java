// Copyright: the support package carries the mapping repository's license
// (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT).

/**
 * Shared support types for WIT-generated Java declarations (mapping v1).
 *
 * <p>NaN note: WIT specifies canonical NaN; Java floating-point NaN
 * comparison semantics differ. Adapters lifting/lowering floats must not
 * rely on bit-exact NaN round-trips.
 */
@WitGenerated(mapping = "v1")
package io.github.witjava.support;
