---
title: "Chapter 4: Include and namespaces"
translationKey: "book-ch04"
---
# Chapter 4: Include and namespaces

As the configuration becomes longer, you may want to split different parts into different files. SML uses `include` to assemble multiple files into a logical whole.

>Design philosophy: From minimalism to richness, functionality can be tailored. Basic include is enabled by default; Complex multi-objective/generic/regular tasks require `@feature enable` to be explicitly enabled to avoid repeating YAML's excessive complexity.

## 4.1 Basic Inline: `Include 'file'`

```sml
# app.sml
include "common.sml"
app: myapp
```

The content of `common.sml` will be pasted in its original form, just like you wrote it by hand. It can also appear inside the block (local injection field):

```sml
database {
    include "conf.d/db.sml"   # 只给 database 块注入字段
    pool: 16
}
```

main points:

-The relative path is parsed according to the directory containing the file itself (consistent with the C preprocessor), and nested inclusion is also intuitive.

-Circular references and missing files will both generate errors and will not be silently skipped.

-The maximum nested level is 32 layers.

## 4.2 namespace: loading files into independent scopes

If there are many keys in `common.sml`, directly inlining them may result in a name collision with the main file. Give it a namespace using `as`:

```sml
include "ui.sml" as ui
# Now all the keys in ui.sml are stored under the UI
title: ui.title
```

`as ui` is equivalent to wrapping the content of ui.sml into the `ui { ... }` block.

### Without extension=default namespace

**Clean rule**: with extension=inline; Without extension=namespace (name takes file name).

```sml
include "ui"          # 等价于 include "ui.sml" as ui
include "ui.sml"      # 内联（带扩展名）
include "ui.sml" as ui.form.widgets   # 显式指定，优先
```

## 4.3 Pointwise path (nested namespace)

`as` supports `a.b.c`, which is equivalent to Rust's module path:

```sml
include "widgets.sml" as ui.form.widgets
```

The expanded logical structure is `ui { form { widgets { ... } } }`. Use prefixes such as `ui.form.widgets.Button` to access.

## 4.4 Macros and contracts are also isolated with namespaces

A namespace not only isolates data, but also isolates fragments from contract definitions. `@contract`/`@name` defined in the included files must be referenced externally with the `ns.` prefix:

```sml
# Widgets. sml Internal
@contract Button { label: str }
@name primary = { label: "OK" }

# When referencing the main file, it must be prefixed
@is ui.form.widgets.Button
button: &ui.form.widgets.primary
```

>The self referencing of its own macros within the file is still resolved by local name (without prefix), and only the `ns.` prefix is required for external exposure. The parser automatically adds prefixes based on the 'namespace stack'.

## 4.5 Multi target and `import` aliases

Separate multiple targets with commas at once; `import` is the equivalent of `include`:

```sml
include "a.sml", "b.sml" as y, "c"
import ui.buttons, admin.panel
```

>Multi target/`import` alias belongs to the "rich layer" and requires `@feature enable multi` to be enabled (see 4.7).

## 4.6 Conflict is an error (not silent)

The namespace is **exclusive scope** and will never silently overwrite:

-Repeatedly defining contracts/fragments with the same name within the same namespace → error.

-Missing file/circular reference/nested beyond 32 layers → error.

## 4.7 Feature layering (customizable)

|Layer | Feature | Ability | Default|
|----|---------|------|------|
|0 | `include` | Basic `include "x.sml"` inline | Open|
|1 | `namespace` | `as ns`+Pointwise Path+Macro/Contract Isolation | Open|
|1 | `implicit-ns` | No extension `include "foo"` ⇒ `as foo` | Open|
|2 | `multi` | comma multi-target, `import` alias | off|
|2 | `glob` | `*` with `dir/*.sml` | Off|
|3 | `regex` | `re:`/`/.../` Regular Matching | Off|
|3 | `ext-rewrite` | `-> .sml` parses non sml files as sml | Off|

Opening method:

```sml
@feature enable glob
include "widgets/*.sml"
```

## 4.8 Zero Copy (Performance Tips)

`include` does not concatenate deep copies of file content into large strings. After each file is read in, only its slice is held, and the parser consumes a "slice stream". When encountering `as a.b`, zero copy open/closed block literals are inserted. The file content is parsed only once, memory=the sum of each file slice. You don't need to worry, but just know it's efficient.

## 4.9 Give it a try with your hands

1. Create `common.sml` and write `@net { region: cn-north-1 timeout: 30 }`.

2. Build `app.sml`:

```sml
   include "common.sml" as cfg
   service {
       region: cfg.net.region
       name: gateway
   }
   ```

3. Analyze `app.sml` and confirm that `service.region` corresponds to `cn-north-1`.

→ [Chapter 5: Contract System](/en/book/ch05 contract)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch04" >}}

{{< sml-quiz "ch04" >}}
