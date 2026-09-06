wit-java mapping v1 — WIT → Java

  bool                      -> boolean
  s8                        -> byte
  s16                       -> short
  s32                       -> int
  s64                       -> long
  u8 / u16 / char           -> int
  u32                       -> long
  u64                       -> long (or java.math.BigInteger via --u64=BigInteger)
  f32                       -> float
  f64                       -> double
  string                    -> java.lang.String
  list<u8> / list<s8>       -> byte[]
  list<T>                   -> java.util.List<T>
  tuple<A..H>               -> <support>.Tuple2..Tuple8 (arity > 8: WJ0002)
  tuple<>                   -> <support>.Unit
  option<T>                 -> java.util.Optional<T> (--option-style=nullable:
                               @Nullable T in non-return positions; nested
                               option: WJ0006)
  result<T,E>               -> <support>.Result<T,E> (absent side: Unit)
  record                    -> record
  enum                      -> enum (UPPER_SNAKE constants)
  variant                   -> sealed interface + nested record per case
  flags                     -> record X(long bits) + or/and/contains/empty()
                               (members > 64: WJ0003)
  type alias / use..as      -> no new type; resolves to the target type
  resource                  -> interface extends AutoCloseable
                               (constructor: nested Factory.create,
                                statics: nested Statics)
  own<T> / borrow<T>        -> T (ownership contract via Javadoc)
  future / stream / error-context
  / fixed-size list<T,N>
  / map<K,V>                -> unsupported in mapping v1 (WJ0001)

Packages: <root>.<ns>.<pkg>.<version-segment>  (1.2.3 -> v1, 0.2.3 -> v0_2)
Worlds:   <pkg>.<world>.<role>.{Imports,Exports}  (--role host|guest|both)

Spec: https://github.com/witjava/wit-java-mapping (spec/v1.md)
