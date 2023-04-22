# Inline diagrams

## Inline diagram in `<kroki>` tag

```
<kroki type="mermaid">
  graph TD
    A[ Anyone ] -->|Can help | B( Go to github.com/yuzutech/kroki )
    B --> C{ How to contribute? }
    C --> D[ Reporting bugs ]
    C --> E[ Sharing ideas ]
    C --> F[ Advocating ]
</kroki>
```

<kroki type="mermaid">
  graph TD
    A[ Anyone ] -->|Can help | B( Go to github.com/yuzutech/kroki )
    B --> C{ How to contribute? }
    C --> D[ Reporting bugs ]
    C --> E[ Sharing ideas ]
    C --> F[ Advocating ]
</kroki>

## Inline diagram in code block

The type of diagram is configured with the language tag: `kroki-<diagram type>`.

``````
```kroki-blockdiag
blockdiag {
  Kroki -> generates -> "Block diagrams";
  Kroki -> is -> "very easy!";

  Kroki [color = "greenyellow"];
  "Block diagrams" [color = "pink"];
  "very easy!" [color = "orange"];
}
```
``````

```kroki-blockdiag
blockdiag {
  Kroki -> generates -> "Block diagrams";
  Kroki -> is -> "very easy!";

  Kroki [color = "greenyellow"];
  "Block diagrams" [color = "pink"];
  "very easy!" [color = "orange"];
}
```
