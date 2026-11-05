# User docs: rules for agents and contributors

Everything in this folder (except this file, CLAUDE.md and README.md) is PIRA's user guide. It
will be published on the PIRA website once that exists; until then the pages read fine on GitHub.
The file path will be the URL, so renaming a file breaks links: add the old path to
`redirect_from` when you rename a published page.

## Who reads this

Artists using PIRA, mostly on a pen tablet, and AI assistants looking things up for them. Write
for an artist, not a developer.

## Page rules

- One question or problem per page. Lead with the answer or the fix in the first two lines.
- Plain words, short sentences, numbered steps for anything the reader does.
- Start the body at `##`. The page title comes from frontmatter.
- Link to other pages instead of repeating them. Use relative links to the `.md` file:
  `[Install PIRA](../getting-started/install.md)`.
- No marketing copy, no "simply", no claims you have not checked in the code or a release.
- Call the product "PIRA". Mention PhotoCraft only to credit it, and send users to PIRA's issue
  tracker, never PhotoCraft's or ArtCraft's.
- Name UI labels exactly as the app shows them, in **bold**. Menu paths read `File › Open…`.
- Troubleshooting pages: `## Symptoms`, `## Fix`, `## Why this happens` (optional),
  `## Still stuck?` (link the issue tracker).

## Frontmatter

    ---
    title: Export an image as PNG or JPEG
    description: One sentence for search results, link previews and AI summaries.
    order: 10                 # optional, sorts within the folder (lower first)
    verified: 0.1.0           # optional, PIRA version you last checked this page against
    redirect_from: [old/path] # optional, old URLs
    ---

Only `key: value` and `key: [a, b]` lines work. Quote values that start with a quote or contain ` #`.

Set `verified` to the latest PIRA release after checking the page against that tag with
`git show <tag>:<path>`. Pages describe the latest release users download, not unreleased code
on main.

## Folders

- `getting-started/` install and first run
- `guides/` how to do one thing
- `troubleshooting/` symptom → fix
- `faq/` short answers
- `reference/` lists and settings

A folder's `index.md` names the folder in the sidebar. Files or folders starting with `_` are
drafts and are not published.
