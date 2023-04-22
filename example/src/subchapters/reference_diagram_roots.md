# Reference diagram roots

## Reference from book root

This is only possible with the `<kroki>` tag.

```md
<kroki type="vegalite" path="book_root_assets/example.vegalite" root="book"/>
```

<kroki type="vegalite" path="book_root_assets/example.vegalite" root="book"/>

## Reference from sources root

This is only possible with the `<kroki>` tag.

```md
<kroki type="c4plantuml" path="sources_root_assets/example.c4plantuml" root="source"/>
```

<kroki type="c4plantuml" path="sources_root_assets/example.c4plantuml" root="source"/>

## Reference from system root

Either of the following will start from the system root.

```md
<kroki type="seqdiag" path="/this/is/probably/a/bad/idea.seqdiag" root="system"/>
```

```md
![My Diagram](/this/is/probably/a/bad/idea.seqdiag)
```

Result not shown, but you get the idea.
