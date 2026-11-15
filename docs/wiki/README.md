# PIRA wiki (source)

Start at [Home](Home.md).

These pages are written as a GitHub wiki: `Home.md` is the front page and `_Sidebar.md` the
sidebar. GitHub wikis need a public repository on the free plan, so until PIRA goes public they
live here. At launch, copy every file except this README into the repository's wiki (its own git
repository, `PIRA.wiki.git`). Wiki links name pages without the extension, so drop `.md` from
the links as you copy them: `sed -i 's/\.md\([)#]\)/\1/g' *.md`.

Until then the wiki is published as part of the website: `python3 web/landing/build_wiki.py` builds
these pages into `web/landing/wiki/`.
