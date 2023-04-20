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
multilingual = false
src = "src"
title = "example"

[preprocessor.kroki-preprocessor]
```

## Usage

There are two ways to use Kroki in your book.

### Fenced code block

You can inline the diagram source into your book with a fenced code block.

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

### `<kroki/>` tag

If the diagram source is too big to inline, you can reference a file using a `<kroki/>` tag. It has
the following attributes:

- `path`: path to file (required)
- `root`: where the path extends from (optional). Possible values:
    - `"system"`: your system's root. Requires `src` to be an absolute path.
    - `"book"`: the book's root. (directory your `book.toml` is in)
    - `"source"`: the sources root. (typically `<book root>/src`, but can be configured in `bool.toml`)
    - `"this"`: the current markdown file. (default if omitted)
- `type`: diagram type (required)

```md
<kroki type="plantuml" root="book" path="/assets/my_diagram.plantuml" />
```

It is recommended to use the self-closing tag syntax `<kroki/>`, but you can use `<kroki></kroki>`
if you want. Anything between the tags will be ignored.

### `![]()` Image tag

The other method is to use an image tag, for diagrams contents that are too big to put inline
in the markdown (such as for excalidraw):

```markdown
![Excalidraw example](kroki-excalidraw:example.excalidraw)
```

The title field can be anything, but the source field needs to start with `kroki-<diagram type>:`.
Both relative and absolute paths are supported. Relative paths are relative to the current markdown
source file, *not* the root of the mdbook. For better configuration of paths, use the `<kroki/>` tag.

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
