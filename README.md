# mdbook-kroki-preprocessor

An mdbook preprocessor that lets you embed diagrams from any of [Kroki's](https://kroki.io)
endpoints into your book.

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

There are two ways to use Kroki in your book. First is a fenced code block:

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

The other method is to use an image tag, for diagrams contents that are too big to put inline
in the markdown (such as for excalidraw):

```markdown
![Excalidraw example](kroki-excalidraw:example.excalidraw)
```

The title field can be anything, but the source field needs to start with `kroki-<diagram type>:`.
Both relative and absolute paths are supported. Relative paths are relative to the current markdown
source file, *not* the root of the mdbook.

The preprocessor will collect all Kroki diagrams of both types, send requests out in parallel
to the appropriate Kroki API endpoint, and replace their SVG contents back into the markdown.

## Endpoint Configuration

If you'd like to use a self-managed instance of Kroki, you can configure the preprocessor to
use a different endpoint:

```toml
[preprocessor.kroki-preprocessor]
endpoint = "https://myurl.com/"
```

The preprocessor will add a trailing slash if needed.

This preprocessor has not been tested on any endpoint other than Kroki's free service.

## Other

This preprocessor only supports HTML rendering.
