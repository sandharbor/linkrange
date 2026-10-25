[[t011 ---- page name with special characters -- before dot . and after dot]]

[[t011 ---- page name with special characters -- single quote ' in name]]

[[test.io which looks like a filename suffix]]

A link whose letter case differs from the page name, which should still find the page: [[T011 ---- Page Name With Special Characters -- Before Dot . And After Dot]]

A link to an alias, which should find the page whose frontmatter lists it under `aliases:` and show the alias as written (no page has this name, so Obsidian Publish falls back to aliases, in any letter case): [[the page with a quote]]

### Links in a table with escaped aliases

First, before we get into the table, let's just do one _outside_ the table:

[[t011/t011 --- pointed to by an escaped alias outside a table\|link with an escaped alias works outside a table]]

| Column 1 | Description Column | Column 3 |
|----------|--------------------|----------|
| [[t011/t011 --- table test row 1 column 1\|table test row 1 column 1]] | This is a test of row 1 | [[t011/t011 --- table test row 1 column 3\|table test row 1 column 3]] |
| [[t011/t011 --- table test row 2 column 1\|table test row 2 column 1]] | This is a test of row 2 | [[t011/t011 --- table test row 2 column 3\|table test row 2 column 3]] |

### Links in unusual places

Obsidian finds a link from `[[` to the first `]]` on the same line, so these should behave as described.

#### A heading that links to [[t011/t011 --- pointed to by an escaped alias outside a table|the escaped-alias page]]

A link whose display text has a single `]`, which is fine because the link ends at the first `]]`: [[t011/t011 --- pointed to by an escaped alias outside a table|display text with a ] in it]]

A link split across two lines, which is not a link, so it should stay as plain text: [[t011/t011 --- pointed to by an escaped alias
outside a table]]

A link and a tag inside an HTML block, where Markdown isn't rendered, so both should stay as plain text:

<div>
[[t011/t011 --- pointed to by an escaped alias outside a table]]
#t011-not-a-tag-inside-html
</div>
