This test covers standard markdown link syntax.

Link to a page in subdirectory: [Linked Page](./t024/t024 ---- linked page.md)

Link to a deeper page: [Deep Page](./t024/deeper/t024 ---- deep page.md)

Link to inlink-only page: [Inlink Only](./t024/t024 ---- inlink only.md)

Link to an image: [Test Image](./t024/t024 ---- test image.png)

Link outside the source graph: [Outside](../../../somewhere/else.md)

External link whose text is **bold** and *italic*: [**bold** and *italic* text](https://example.com/formatted)

External link wrapped around an image, like a README badge: [![badge](https://img.shields.io/badge/meadow-test-green.svg)](https://example.com/badge)

External link with a title containing quotes: [titled](https://example.com/titled 'A "quoted" title')

The next three links are other standard ways of writing the first link on this page. Each should lead to the same linked page.

A reference-style link, with its destination defined at the bottom of the page: [Linked Page][linked page]

A link with a title: [Linked Page](./t024/t024%20----%20linked%20page.md "The linked page")

A link whose destination is in angle brackets, which allows spaces: [Linked Page](<./t024/t024 ---- linked page.md>)

A link with the page's file name and no folder, which Obsidian finds in whatever folder the page is in: [Linked Page](t024%20----%20linked%20page.md)

[linked page]: ./t024/t024%20----%20linked%20page.md
