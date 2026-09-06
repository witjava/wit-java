// Copyright: the support package carries the mapping repository's license
// (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT).

package {{PACKAGE}};

/** A WIT {@code result<T, E>}: either {@link Ok} or {@link Err}. */
public sealed interface Result<T, E> permits Result.Ok, Result.Err {

    /** Wrap a success value. */
    static <T, E> Result<T, E> ok(T value) {
        return new Ok<>(value);
    }

    /** Wrap an error value. */
    static <T, E> Result<T, E> err(E error) {
        return new Err<>(error);
    }

    /** The success case. */
    record Ok<T, E>(T value) implements Result<T, E> {}

    /** The error case. */
    record Err<T, E>(E error) implements Result<T, E> {}
}
