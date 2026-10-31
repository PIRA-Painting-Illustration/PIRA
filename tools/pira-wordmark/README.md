# PIRA wordmark

The chrome "PIRA" wordmark is drawn from shapes, not a font (`geo_mark.py`, style `linebold`,
italic), then given depth in `mark3d.py` (treatment D, chrome). `chrome_set.py` writes the full set
of lockups. The tagline uses Montserrat (SIL Open Font License), fetched from Google Fonts into a
fonts folder (`montserrat-Montserrat[wght].ttf`).

    python chrome_set.py <fonts dir> ../../assets/app-icon/pira-logo-round.png <out dir>

Needs Pillow. The images the app and README use live in `assets/brand/`.
