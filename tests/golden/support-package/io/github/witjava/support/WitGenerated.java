// Copyright: the support package carries the mapping repository's license
// (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT).

package io.github.witjava.support;

/** Marks WIT-generated packages and types. */
@java.lang.annotation.Documented
@java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.RUNTIME)
@java.lang.annotation.Target({java.lang.annotation.ElementType.PACKAGE,
        java.lang.annotation.ElementType.TYPE})
public @interface WitGenerated {
    /** Mapping version, e.g. {@code "v1"}. */
    String mapping();
}
