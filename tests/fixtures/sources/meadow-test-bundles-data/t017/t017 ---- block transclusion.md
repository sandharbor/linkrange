### Block Transclusion Test

This page tests block transclusion. Only the specific block with the key-insight identifier should be transcluded:

![[t017 ---- block source#^key-insight]]

The transclusion should appear above this line.

Here's another block transclusion with a different identifier:

![[t017 ---- block source#^f4c4d5]]

This second transclusion should also appear above this line.

This embeds the block with identifier `key`. It should not be confused with the `key-insight` block, whose identifier starts the same way:

![[t017 ---- block source#^key]]

This embeds only the second item of a list, not the whole list:

![[t017 ---- block source#^second-item]]
