## PNG

![[t006/t006 --- meadow.png]]

![[t006/t006 --- meadow.png|100]]

| test embed in table                |
| ---------------------------------- |
| ![[t006/t006 --- meadow.png\|100]] |

## Image Sizes

Obsidian sizes an image with a width (`|100`, above) or a width and height (`|100x50`, here):

![[t006/t006 --- meadow.png|100x50]]

In Markdown image syntax the size goes where the alt text goes, optionally after the alt text and a `|`. A local image, 80 pixels wide, with alt text:

![A meadow|80](t006/t006%20---%20meadow.png)

An external image, 250 pixels wide, with no alt text:

![250](https://example.com/meadow-external.png)

## Animated GIF

![[t006/t006 --- meadow-flower-petals-ani.gif]]

![[t006/t006 --- meadow-flower-petals-ani.gif|100]]

| test embed in table                                  |
| ---------------------------------------------------- |
| ![[t006/t006 --- meadow-flower-petals-ani.gif\|100]] |

## Too Big PNG

![[t006/t006 --- too-big.png]]

![[t006/t006 --- too-big.png|100]]

| test embed in table                 |
| ----------------------------------- |
| ![[t006/t006 --- too-big.png\|100]] |

## SVG

![[t006/t006 --- meadow-flower.svg]]

![[t006/t006 --- meadow-flower.svg|78]]

| test embed in table                       |
| ----------------------------------------- |
| ![[t006/t006 --- meadow-flower.svg\|100]] |

with a **container directive** that makes the SVG's links interactive

:::meadow
![[t006/t006 --- meadow-flower.svg|300]]

enableEmbeddedLinks: true
enableFullscreenButton: true
enableOpenDedicatedPage: false
:::

## Excalidraw

![[t006 --- meadow-flower.excalidraw]]

with a **container directive** that causes the links to be functional

:::meadow
![[t006 --- meadow-flower.excalidraw|300]]

enableEmbeddedLinks: true
enableFullscreenButton: true
enableOpenDedicatedPage: false
:::

| test embed in table                         |
| ------------------------------------------- |
| ![[t006 --- meadow-flower.excalidraw\|100]] |
[[t006 --- page that embeds Excalidraw in another directory]]

## Audio, video, and PDF

Obsidian embeds audio and video with a player. These are a quarter-second tone and two seconds of green, kept small on purpose. The video is displayed at 320 by 180 pixels so its player controls are usable.

![[t006/t006 --- short tone.mp3]]

![[t006/t006 --- short clip.mp4|320x180]]

A PDF embeds as a viewer, here opened at its first page and 200 pixels high. Its links work, and one goes to a page in this bundle, so that page is in the graph too. The icon at the top right opens the PDF's own page.

![[t006/t006 --- document with a link.pdf#page=1&height=200]]

A container directive can give the viewer a fullscreen button:

:::meadow
![[t006/t006 --- document with a link.pdf#height=160]]

enableFullscreenButton: true
:::
