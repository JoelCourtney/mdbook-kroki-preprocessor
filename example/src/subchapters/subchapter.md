# Excalidraw example

Importing an excalidraw drawing from a saved file:

```md
![Excalidraw example](kroki-excalidraw:example.excalidraw)
```

![Excalidraw example](kroki-excalidraw:example.excalidraw)

<kroki type="plantuml">
@startuml
    skinparam ranksep 20
    skinparam dpi 125
    skinparam packageTitleAlignment left

    rectangle "Main" {
        (main.view)
        (singleton)
    }
    rectangle "Base" {
        (base.component)
        (component)
        (model)
    }
    rectangle "<b>main.ts</b>" as main_ts

    (component) ..> (base.component)
    main_ts ==> (main.view)
    (main.view) --> (component)
    (main.view) ...> (singleton)
    (singleton) ---> (model)
@enduml
</kroki>
