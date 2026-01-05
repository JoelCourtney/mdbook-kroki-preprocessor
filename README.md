# mdbook-kroki-preprocessor

An mdbook preprocessor that lets you embed diagrams from any of [Kroki's](https://kroki.io)
diagram types into your book.

## Setup

First install this preprocessor with `cargo install mdbook-kroki-preprocessor`.

Then add the preprocessor to your `book.toml`:

```toml
[book]
authors = ["You"]
language = "en"
src = "src"
title = "example"

[preprocessor.kroki-preprocessor]
```

## Usage

Diagram code can either be inlined in your markdown or referenced in an external file, and both methods can either be
done with a special `<kroki>` tag or traditional markdown elements. In most cases the `<kroki>` tag is recommended for readability.

For examples inside a real mdbook, see [the example dir](example).

### `<kroki>` tag

You can inline your code in the `<kroki>` tag like this:

```md
<kroki type="erd">
  [Person]
  *name
  height
  weight
  +birth_location_id

  [Location]
  *id
  city
  state
  country

  Person *--1 Location 
</kroki>
```

The `type` attribute tells kroki what renderer to use and is required.

If the code is too big to fit inline neatly, you can reference an external file like this:

```md
<kroki type="plantuml" root="book" path="/assets/my_diagram.plantuml" />
```

The possible attributes are:

- `type`: diagram type (required)
- `path`: path to file (optional)
- `root`: where the path extends from (optional). Possible values:
  - `"system"`: your system's root. Requires `src` to be an absolute path.
  - `"book"`: the book's root. (directory your `book.toml` is in)
  - `"source"`: the sources root. (typically `<book root>/src`, but can be configured in `bool.toml`)
  - `"this"`: the current markdown file. (default if omitted)

When referencing a file it is recommended to use the self-closing tag syntax `<kroki/>`, but you can use `<kroki></kroki>`
if you want. Anything between the tags will be ignored if the `path` attribute is present.

### Fenced code block

If you want to use traditional markdown elements, you can inline the diagram source into your book with a fenced code block.

``````markdown
```kroki-mermaid
graph TD
  A[ Anyone ] -->|Can help | B( Go to github.com/yuzutech/kroki )
  B --> C{ How to contribute? }
  C --> D[ Reporting bugs ]
  C --> E[ Sharing ideas ]
  C --> F[ Advocating ]
```
``````

The code block's language has to be `kroki-<diagram type>`.

### `![]()` Image tag

Or you can reference an external file using a markdown image tag:

```markdown
![Excalidraw example](kroki-excalidraw:example.excalidraw)
```

The title field can be anything, but the source field needs to start with `kroki-<diagram type>:`.
Both relative and absolute paths are supported. Relative paths are relative to the current markdown
source file, *not* the root of the mdbook. Absolute paths are from the system root.
For better configuration of paths, use the `<kroki/>` tag.

## Endpoint Configuration

If you'd like to use a self-managed instance of Kroki, you can configure the preprocessor to
use a different endpoint:

```toml
[preprocessor.kroki-preprocessor]
endpoint = "http://localhost:8000"
```

The preprocessor will add a trailing slash if needed. The default is "<https://kroki.io/>".

## Other

This preprocessor only supports HTML rendering.
