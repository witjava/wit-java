// Tier 1 conformance checker for the wit-java mapping spec.
//
// JDK-only (no third-party dependencies). Parses (does not compile) every
// .java file under a directory with the Compiler Tree API (com.sun.source)
// and emits a canonical API-shape digest to stdout:
//
//   - one "package <fqn>" line per package (sorted)
//   - one line per type, sorted by name: kind, type parameters, record
//     components (source order), supertypes, permits (source order)
//   - one indented line per member: methods sorted by signature, enum
//     constants in source order
//
// Deliberately NOT compared (file organization is not part of the API):
//   - file split, file names, import statements, comments/Javadoc prose,
//     the @generated header, declaration order, whitespace, line endings
//
// Type references are canonicalized to fully-qualified names using the
// import statements of the declaring file plus the set of types declared in
// the parsed tree, so a simple-name style and an FQN style digest identically.
//
// Usage:
//   java tools/checker/Checker.java <dir>            print digest
//   java tools/checker/Checker.java --compare A B    exit 0 if digests equal
//   java tools/checker/Checker.java --selftest       run built-in self-test

import com.sun.source.tree.*;
import com.sun.source.util.JavacTask;
import com.sun.source.util.Trees;
import com.sun.source.util.TreeScanner;

import javax.lang.model.element.ElementKind;
import javax.lang.model.element.Modifier;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import java.util.stream.Collectors;
import java.util.stream.Stream;

public final class Checker {

    private static final String HEADER = "#wit-java-tier1-digest-v1";

    // java.lang simple names that may appear unqualified without an import.
    private static final Set<String> JAVA_LANG = Set.of(
            "String", "Object", "Integer", "Long", "Boolean", "Byte", "Short",
            "Float", "Double", "Character", "Number", "Comparable", "Iterable",
            "Runnable", "AutoCloseable", "Exception", "RuntimeException",
            "IllegalArgumentException", "IllegalStateException",
            "UnsupportedOperationException", "Override", "Deprecated",
            "SuppressWarnings", "FunctionalInterface", "Documented",
            "Retention", "Target", "ElementType", "RetentionPolicy", "Void");

    /** Per-compilation-unit resolution context. */
    private static final class Ctx {
        String pkg = "";
        final Map<String, String> imports = new HashMap<>();   // simple -> fqn
        final Map<String, String> declared = new HashMap<>();  // simple -> fqn (this file)
    }

    private Checker() {}

    public static void main(String[] args) throws IOException {
        if (args.length == 1 && !args[0].startsWith("-")) {
            digestDir(Path.of(args[0])).forEach(System.out::println);
        } else if (args.length == 3 && args[0].equals("--compare")) {
            List<String> a = digestDir(Path.of(args[1]));
            List<String> b = digestDir(Path.of(args[2]));
            if (a.equals(b)) {
                System.out.println("OK: digests equal (" + (a.size() - 1) + " lines)");
            } else {
                System.out.println("MISMATCH:");
                int n = Math.max(a.size(), b.size());
                int shown = 0;
                for (int i = 0; i < n && shown < 20; i++) {
                    String la = i < a.size() ? a.get(i) : "<eof>";
                    String lb = i < b.size() ? b.get(i) : "<eof>";
                    if (!la.equals(lb)) {
                        System.out.println("  A: " + la);
                        System.out.println("  B: " + lb);
                        shown++;
                    }
                }
                System.exit(1);
            }
        } else if (args.length == 1 && args[0].equals("--selftest")) {
            System.exit(selftest() ? 0 : 1);
        } else {
            System.err.println("usage: Checker.java <dir> | --compare A B | --selftest");
            System.exit(2);
        }
    }

    // ---------------------------------------------------------------- digest

    static List<String> digestDir(Path dir) throws IOException {
        if (!Files.isDirectory(dir)) {
            throw new IOException("not a directory: " + dir);
        }
        List<Path> files;
        try (Stream<Path> s = Files.walk(dir)) {
            files = s.filter(p -> p.toString().endsWith(".java"))
                     .sorted(Comparator.comparing(p -> dir.relativize(p).toString()))
                     .collect(Collectors.toList());
        }
        if (files.isEmpty()) {
            return List.of(HEADER, "(empty)");
        }

        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        StandardJavaFileManager fm =
                compiler.getStandardFileManager(null, Locale.ROOT, StandardCharsets.UTF_8);
        Iterable<? extends JavaFileObject> units = fm.getJavaFileObjects(
                files.toArray(new Path[0]));
        JavacTask task = (JavacTask) compiler.getTask(
                null, fm, null, List.of("-proc:none"), null, units);
        try {
            Iterable<? extends CompilationUnitTree> asts = task.parse();
            Trees trees = Trees.instance(task);

            // pass 1: contexts (package, imports, declared names) + emission data
            Map<String, String> packageAnnos = new TreeMap<>();
            List<Ctx> ctxs = new ArrayList<>();
            List<List<String>> typeBlocks = new ArrayList<>();
            for (CompilationUnitTree cu : asts) {
                Ctx ctx = new Ctx();
                ExpressionTree pkgName = cu.getPackageName();
                if (pkgName != null) {
                    ctx.pkg = pkgName.toString();
                    String anns = cu.getPackageAnnotations().stream()
                            .map(a -> "@" + simpleName(a))
                            .filter(n -> !n.isEmpty())
                            .sorted()
                            .collect(Collectors.joining(" "));
                    if (!anns.isEmpty()) {
                        packageAnnos.merge(ctx.pkg, anns, (a, b) -> a + " " + b);
                    }
                }
                for (ImportTree imp : cu.getImports()) {
                    if (imp.isStatic()) {
                        continue;
                    }
                    String fqn = imp.getQualifiedIdentifier().toString();
                    int dot = fqn.lastIndexOf('.');
                    if (dot > 0) {
                        ctx.imports.put(fqn.substring(dot + 1), fqn);
                    }
                }
                new TreeScanner<Void, Void>() {
                    @Override public Void visitClass(ClassTree t, Void unused) {
                        ctx.declared.put(t.getSimpleName().toString(),
                                ctx.pkg + "." + t.getSimpleName());
                        return super.visitClass(t, unused);
                    }
                }.scan(cu, null);
                ctxs.add(ctx);
            }

            // pass 2: canonicalize cross-file references (same-package types),
            // group by package, sort.
            Map<String, Set<String>> packageTypes = new HashMap<>();
            for (Ctx c : ctxs) {
                c.declared.values().forEach(fqn -> {
                    int dot = fqn.lastIndexOf('.');
                    packageTypes.computeIfAbsent(fqn.substring(0, dot), k -> new HashSet<>())
                                .add(fqn.substring(dot + 1));
                });
            }
            setPackageTypes(packageTypes);
            TreeMap<String, List<String>> byPackage = new TreeMap<>();
            Iterator<? extends CompilationUnitTree> astIt = asts.iterator();
            for (Ctx c : ctxs) {
                CompilationUnitTree cu = astIt.next();
                for (String line : emitTypes(cu, c, trees)) {
                    byPackage.computeIfAbsent(c.pkg, k -> new ArrayList<>()).add(line);
                }
            }
            List<String> out = new ArrayList<>();
            out.add(HEADER);
            for (Map.Entry<String, List<String>> e : byPackage.entrySet()) {
                String panns = packageAnnos.get(e.getKey());
                out.add("package " + e.getKey()
                        + (panns == null || panns.isEmpty() ? "" : " " + panns));
                // group lines into blocks: a block starts at a line NOT
                // beginning with two spaces (a type line); its indented
                // members stay attached and keep their internal order.
                List<List<String>> blocks = new ArrayList<>();
                for (String line : e.getValue()) {
                    if (!line.startsWith("  ") || blocks.isEmpty()) {
                        blocks.add(new ArrayList<>());
                    }
                    blocks.get(blocks.size() - 1).add(line);
                }
                blocks.sort(Comparator.comparing(b -> b.get(0)));
                for (List<String> b : blocks) {
                    out.addAll(b);
                }
            }
            return out;
        } finally {
            fm.close();
        }
    }

    /** Emits sorted type lines (members included, already sorted within type). */
    private static List<String> emitTypes(CompilationUnitTree cu, Ctx ctx, Trees trees) {
        List<String> lines = new ArrayList<>();
        for (Tree decl : cu.getTypeDecls()) {
            if (decl instanceof ClassTree ct) {
                emitClass(ct, ctx, "", lines);
            }
        }
        return lines;
    }

    private static void emitClass(ClassTree ct, Ctx ctx, String outer, List<String> lines) {
        String simple = ct.getSimpleName().toString();
        String name = outer.isEmpty() ? simple : outer + "." + simple;
        Set<Modifier> mods = ct.getModifiers().getFlags();
        String kind = switch (ct.getKind()) {
            case RECORD -> "record";
            case ENUM -> "enum";
            case ANNOTATION_TYPE -> "@interface";
            case INTERFACE -> mods.contains(Modifier.SEALED)
                    ? "sealed interface" : "interface";
            default -> mods.contains(Modifier.ABSTRACT) ? "abstract class" : "class";
        };
        StringBuilder sb = new StringBuilder();
        sb.append(kind).append(' ').append(name);
        if (!ct.getTypeParameters().isEmpty()) {
            sb.append(ct.getTypeParameters().stream()
                    .map(tp -> tp.toString().trim())
                    .collect(Collectors.joining(", ", "<", ">")));
        }
        if (ct.getKind() == Tree.Kind.RECORD) {
            String comps = ct.getMembers().stream()
                    .filter(m -> m instanceof VariableTree v
                            && !v.getModifiers().getFlags().contains(Modifier.STATIC))
                    .map(m -> recordComponent((VariableTree) m, ctx))
                    .collect(Collectors.joining(", "));
            sb.append('(').append(comps).append(')');
        }
        if (ct.getExtendsClause() != null) {
            sb.append(" extends ").append(canon(ct.getExtendsClause(), ctx));
        }
        if (!ct.getImplementsClause().isEmpty()) {
            sb.append(" implements ").append(ct.getImplementsClause().stream()
                    .map(t -> canon(t, ctx)).sorted()
                    .collect(Collectors.joining(", ")));
        }
        if (ct.getPermitsClause() != null && !ct.getPermitsClause().isEmpty()) {
            sb.append(" permits ").append(ct.getPermitsClause().stream()
                    .map(t -> canon(t, ctx))
                    .collect(Collectors.joining(", ")));
        }
        lines.add(sb.toString());

        // members: methods sorted; enum constants in order; nested types recursive
        List<String> methods = new ArrayList<>();
        for (Tree member : ct.getMembers()) {
            if (member instanceof MethodTree mt && mt.getReturnType() != null) {
                methods.add("  " + name + " :: " + methodSig(mt, ctx));
            } else if (member instanceof VariableTree vt) {
                boolean isEnumConstant = ct.getKind() == Tree.Kind.ENUM;
                if (isEnumConstant) {
                    lines.add("  " + name + " :: constant " + vt.getName());
                } else if (vt.getModifiers().getFlags().contains(Modifier.STATIC)) {
                    String anns = varAnnotations(vt);
                    lines.add("  " + name + " :: static field "
                            + (anns.isEmpty() ? "" : anns + " ")
                            + canon(vt.getType(), ctx) + " " + vt.getName());
                }
            } else if (member instanceof ClassTree nested) {
                emitClass(nested, ctx, name, lines);
            }
        }
        Collections.sort(methods);
        lines.addAll(methods);
    }

    private static String varAnnotations(VariableTree v) {
        return v.getModifiers().getAnnotations().stream()
                .map(a -> simpleName(a))
                .filter(n -> !n.isEmpty())
                .map(n -> "@" + n)
                .sorted()
                .collect(Collectors.joining(" "));
    }

    private static String recordComponent(VariableTree v, Ctx ctx) {
        String anns = varAnnotations(v);
        return (anns.isEmpty() ? "" : anns + " ")
                + canon(v.getType(), ctx) + " " + v.getName();
    }

    private static String methodSig(MethodTree mt, Ctx ctx) {
        Set<Modifier> mods = mt.getModifiers().getFlags();
        StringBuilder sb = new StringBuilder();
        if (mods.contains(Modifier.STATIC)) {
            sb.append("static ");
        }
        mt.getModifiers().getAnnotations().stream()
                .map(a -> simpleName(a))
                .filter(n -> !n.isEmpty())
                .forEach(n -> sb.append('@').append(n).append(' '));
        sb.append(mt.getName()).append('(');
        sb.append(mt.getParameters().stream()
                .map(p -> canon(p.getType(), ctx) + " " + p.getName())
                .collect(Collectors.joining(", ")));
        sb.append(") -> ").append(canon(mt.getReturnType(), ctx));
        return sb.toString();
    }

    private static String simpleName(Tree annotation) {
        String s = annotation.toString();
        // @Override is not part of the API surface
        if (s.startsWith("@Override")) {
            return "";
        }
        int paren = s.indexOf('(');
        if (paren > 0) {
            s = s.substring(0, paren);
        }
        s = s.startsWith("@") ? s.substring(1) : s;
        int dot = s.lastIndexOf('.');
        return dot > 0 ? s.substring(dot + 1) : s;
    }

    // ------------------------------------------------- type canonicalization

    static String canon(Tree t, Ctx ctx) {
        if (t == null) {
            return "void";
        }
        if (t instanceof PrimitiveTypeTree p) {
            return p.getPrimitiveTypeKind().toString().toLowerCase();
        }
        if (t instanceof ArrayTypeTree at) {
            return canon(at.getType(), ctx) + "[]";
        }
        if (t instanceof ParameterizedTypeTree pt) {
            return canon(pt.getType(), ctx)
                    + pt.getTypeArguments().stream()
                        .map(a -> canon(a, ctx))
                        .collect(Collectors.joining(", ", "<", ">"));
        }
        if (t instanceof AnnotatedTypeTree ann) {
            String anns = ann.getAnnotations().stream()
                    .map(a -> "@" + simpleName(a))
                    .sorted()
                    .collect(Collectors.joining(" "));
            return (anns.isEmpty() ? "" : anns + " ") + canon(ann.getUnderlyingType(), ctx);
        }
        if (t instanceof MemberSelectTree ms) {
            String full = t.toString().replace(" ", "");
            // longest import match, then suffix
            for (String fqn : ctx.imports.values()) {
                if (full.equals(fqn)) {
                    return fqn;
                }
                if (full.startsWith(fqn + ".")) {
                    return fqn + full.substring(fqn.length());
                }
            }
            // leftmost segment resolves via imports
            String left = ms.getExpression().toString();
            String leftCanon = left.contains(".")
                    ? null
                    : resolveSimple(left, ctx);
            if (leftCanon != null && !leftCanon.equals(left)) {
                return leftCanon + "." + ms.getIdentifier();
            }
            return full;
        }
        if (t instanceof IdentifierTree id) {
            String n = id.getName().toString();
            String r = resolveSimple(n, ctx);
            return r != null ? r : n;
        }
        return t.toString();
    }

    private static String resolveSimple(String n, Ctx ctx) {
        if (ctx.imports.containsKey(n)) {
            return ctx.imports.get(n);
        }
        if (ctx.declared.containsKey(n)) {
            return ctx.declared.get(n);
        }
        if (packageContains(ctx.pkg, n)) {
            return ctx.pkg + "." + n;
        }
        if (JAVA_LANG.contains(n)) {
            return "java.lang." + n;
        }
        return null;
    }

    // same-package resolution for pass 2; filled by digestDir before canonicalizing
    private static final ThreadLocal<Map<String, Set<String>>> PACKAGE_TYPES =
            ThreadLocal.withInitial(HashMap::new);

    static void setPackageTypes(Map<String, Set<String>> m) {
        PACKAGE_TYPES.get().clear();
        PACKAGE_TYPES.get().putAll(m);
    }

    private static boolean packageContains(String pkg, String simple) {
        Set<String> s = PACKAGE_TYPES.get().get(pkg);
        return s != null && s.contains(simple);
    }

    // -------------------------------------------------------------- selftest

    private static boolean selftest() throws IOException {
        Path base = Files.createTempDirectory("wj-checker-selftest");
        // A: simple-name style, two files
        write(base.resolve("a/p/A.java"), """
                package p;
                import io.github.witjava.support.Result;
                public interface A {
                    Result<Integer, String> f(int x);
                    byte[] g(java.util.List<io.github.witjava.support.Tuple2<Integer, String>> xs);
                }
                """);
        write(base.resolve("a/p/B.java"), """
                package p;
                import java.util.List;
                import io.github.witjava.support.Tuple2;
                public record B(int v0, @Nullable String v1) {}
                """);
        // B: FQN style, single file, members reordered
        write(base.resolve("b/p/all.java"), """
                package p;
                public record B(int v0, @io.github.witjava.support.Nullable String v1) {}
                public interface A {
                    byte[] g(java.util.List<io.github.witjava.support.Tuple2<Integer, String>> xs);
                    io.github.witjava.support.Result<java.lang.Integer, java.lang.String> f(int x);
                }
                """);
        List<String> da = digestDir(base.resolve("a"));
        List<String> db = digestDir(base.resolve("b"));
        boolean equal = da.equals(db);
        System.out.println("selftest A==B: " + (equal ? "PASS" : "FAIL"));
        if (!equal) {
            da.forEach(l -> System.out.println("  A| " + l));
            db.forEach(l -> System.out.println("  B| " + l));
        }
        // C: one changed parameter type must differ
        write(base.resolve("c/p/A.java"), """
                package p;
                import io.github.witjava.support.Result;
                public interface A {
                    Result<Integer, String> f(long x);
                }
                """);
        List<String> dc = digestDir(base.resolve("c"));
        boolean differs = !da.equals(dc);
        System.out.println("selftest A!=C: " + (differs ? "PASS" : "FAIL"));
        return equal && differs;
    }

    private static void write(Path p, String content) throws IOException {
        Files.createDirectories(p.getParent());
        Files.writeString(p, content, StandardCharsets.UTF_8);
    }
}
