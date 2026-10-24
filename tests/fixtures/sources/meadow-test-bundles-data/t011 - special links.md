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
