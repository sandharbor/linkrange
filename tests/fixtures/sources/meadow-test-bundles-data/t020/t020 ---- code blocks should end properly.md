To do this, we need a type that represents non-empty lists. Fortunately, the existing `NonEmpty` type from `Data.List.NonEmpty` is exactly that. It has the following definition:
:
```haskell
data NonEmpty a = a :| [a]
```
......
The dots line above should not be included in the haskell code block

A lone backtick does not start inline code: press the ` key to open the console.

So this link, between two lone backticks in separate paragraphs, should still work: [[t020 - code blocks]]

This sentence ends with another lone backtick: `
