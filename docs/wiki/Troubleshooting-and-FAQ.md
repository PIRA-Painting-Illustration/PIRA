# Troubleshooting and FAQ

## Is PIRA free?

Yes. No subscription, no account, no ads, and no features locked behind a paid tier. It works
offline: you never need to log in or connect to draw, save or export.

## Is it open source?

PIRA is built on PhotoCraft, which is open source under the MIT or Apache 2.0 licence.

## Which devices does it run on?

Android tablets (64-bit ARM), Windows 10 and 11 (64-bit), and Linux (x86_64). There's no Mac
build. PIRA is tested on Android tablets first.

## My strokes ignore pen pressure

1. Check **Pressure for Size** and **Pressure for Opacity** in the options bar.
2. Try another brush: some brushes are set to ignore pressure.
3. On Android, open **Window › Touch Input › Touch Input Settings…** and press the pen on the
   screen: the pen should be reported as a pen, not a finger.
4. On Windows, make sure the tablet's driver is installed; PIRA reads pens through Windows' own
   pen support and through WinTab.

## My palm draws on the canvas

Palm rejection ignores fingers while the pen is down or near the screen. If a palm still draws,
set what one finger does to **Move** or **Nothing** in **Window › Touch Input**.

## The fill leaks into the next area

Raise **Close Gap** on the Paint Bucket, or close the gap in the line. See
[Line art and colouring](Line-Art-and-Colouring.md).

## A menu item is greyed out

It needs something that isn't there: an open canvas, a selection, a particular kind of layer.
**Export Timelapse** needs a recording; it tells you what's missing when you choose it.

## Windows says the publisher is unknown

PIRA isn't code-signed yet. Choose **More info › Run anyway**.

## Where are my settings?

| System | Folder |
|---|---|
| Android | The app's own storage (removed if you uninstall) |
| Windows | `%APPDATA%\PIRA` |
| Linux | `~/.config/pira` |
| Portable copy | `PIRAData` next to the app (when `portable.txt` is there) |

## Reporting a problem

Open an issue in PIRA's [issue tracker](https://github.com/PIRA-Painting-Illustration/PIRA/issues),
or use **Help › Report an Issue…** in the app. Say which device and version you're on
(**Help › About**), and what you did just before the problem.
