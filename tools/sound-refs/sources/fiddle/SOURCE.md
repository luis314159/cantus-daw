# Country / old-time / bluegrass fiddle references (round 42, the country fiddle)

For measurement and the owner's ear only (`use = "measure"`): nothing here ships. Every file's
source page, download URL, author, licence and description, as the source states them, is in
`manifest.json` beside this file. **Access date:** 2026-11-25 (every item).

## 1. Modern recordings: Freesound previews (`freesound/`)

- Freesound serves the public `-hq.mp3` previews without a login (the originals need one). These
  are lossy (MP3, about 128 kbit/s, content to about 16 kHz): fine for everything measured below
  10 kHz.
- **TheFlyFishingFilmmaker**, 12 sounds (621590-621599, 641690, 641699, 641703): bluegrass fiddle,
  a hornpipe, bluesy licks, double stops. Each sound page shows the licence "Attribution 4.0"
  and links `https://creativecommons.org/licenses/by/4.0/` (CC BY 4.0). One player, one fiddle.
  Page text, verbatim: 621593 "Part of a bluegrass song"; 621592 "standard bluegrass, folk, old
  time fiddle ending"; 621598 "this repeating bluesy double stops on the violin would be good as
  to build tension in a video".
- **BoilingSand**, 49783 "080313 fiddle run on G major scale": "Attribution" with
  `http://creativecommons.org/licenses/by/3.0/` (CC BY 3.0). Page text: "Short ascending/descending
  run in G major (or A minor, if you prefer it) played on 1920s fiddle in 10' x 10' room recorded
  from 3-4' away into front mics of Zoom PS-02."
- **ecfike**, 135128 "Denver Belle": "Creative Commons 0" with
  `http://creativecommons.org/publicdomain/zero/1.0/` (CC0). Page text: "My fiddle teacher playing
  guitar backup as yours truely plays the violin rather poorly." A student with guitar backing:
  timing only.

## 2. Modern recordings and classical violin: Wikimedia Commons (`commons/`)

The licence as the file page states it (the API's `LicenseShortName` / `UsageTerms`):

- `Willy_Wagtail.ogg` (English fiddle tune, YouTube user 'peakfiddler', 2019-05-08): "CC BY 3.0",
  "Creative Commons Attribution 3.0".
- `The_Devil_s_Dream.ogg` ("US fiddle tune The Devil's Dream", CambridgeBayWeather): "CC BY-SA
  3.0", "Creative Commons Attribution-Share Alike 3.0".
- `Violin_double_stop.ogg`, `Violin_chords.ogg`, `Violin_G_major_scale.ogg` (Clngre, 2005/2007):
  "CC BY-SA 3.0". Used to check the double-stop detector.
- `Violin_sounds_and_techniques.ogg` (Alex Dee, 2006-07-09): "CC BY 2.5", "Creative Commons
  Attribution 2.5".
- `Bach_Chaconne_BWV1004_Ben_Goldstein.ogg` (File:Johann Sebastian Bach - Chaconne for violin
  alone.ogg, "Performed by Ben Goldstein"): "CC BY-SA 3.0". Classical double stops.

## 3. Public-domain 78s (`pd78/`)

- **The line on 2026-11-25:** under the Music Modernization Act (17 U.S.C. sec. 1401(a)(2)), a
  sound recording first published before 1923 left protection on 2022-01-01, and one published
  1923-1946 is protected 100 years from publication (to the end of that year). So 1923
  recordings entered the US public domain on 2024-01-01, 1924 on 2025-01-01 and 1925 on
  2026-01-01: **today every recording first published in 1925 or earlier is in the US public
  domain**; 1926 recordings follow on 2027-01-01 (so Charlie Poole's "White House Blues", 1926,
  was left out although Commons tags it public domain).
- From the Internet Archive (the item's own licence field, where it has one, is quoted in
  `manifest.json`; the public-domain status rests on the publication date above):
  - Eck Robertson, "Sallie Gooden" (recorded 1922-07-01, Victor 18956, issued 1923; item
    `ACEckRobertsonSallieGoodenVictor18956`, FLAC): **unaccompanied solo fiddle**.
  - Henry C. Gilliland and A. C. (Eck) Robertson, "Arkansaw Traveler" (recorded June 1922, Victor 18956;
    item `Gilliland_and_Robertson-Arkansaw_Traveler`): two fiddles.
  - Eck Robertson, "Ragtime Annie" (1922; item `Eck_Robertson_Ragtime_Annie`): fiddle with piano.
  - Don Richardson, "Mississippi Sawyer" (1916-04; item `Don_Richardson_Mississippi_Sawyer_1916`).
  - Fiddlin' Powers and Family, "Cripple Creek" (1924-08-19, Victor 19449-A; item
    `cripple-creek_202509`, licenseurl CC0 by the uploader): a string band.
  - Eck Robertson, "Done Gone" and "Sallie Johnson" (1922, with Nathaniel Shilkret; Library of
    Congress National Jukebox transfers on the Archive, items `loc-jukebox-63079-done-gone`,
    `loc-jukebox-63074-sallie-johnson`). Their rights field, verbatim: "Public domain. Sound
    recording first published 1922; statutory protection ended per Music Modernization Act, 17
    U.S.C. sec. 1401 (100 years after publication + transition)."
- **What they can and cannot show:** acoustic (horn) recordings, band-limited to about
  200-4000 Hz with surface noise and an unknown turntable speed. They are measured for timing,
  pitch and rhythm only (tempo, swing, accents, slides, intonation relative to the tune's own
  tonic), never for timbre or noise.

## Not fetched

- loc.gov (the American Folklife Center's "Fiddle Tunes of the Old Frontier: The Henry Reed
  Collection") answers automated clients with a Cloudflare browser challenge (HTTP 403, "Just a
  moment..."): not worked around. A hand download from https://www.loc.gov/collections/henry-reed/
  (check each item's rights statement) would add superb unaccompanied old-time fiddle.
- Commons `SallieJohnson&BillyInTheLowground.ogg` (Eck Robertson, 1922, public domain): the
  server answered HTTP 429 (rate limit) twenty times; the same 1922 "Sallie Johnson" is here from
  the Library of Congress transfer.
- Freesound and Commons hold little else that is solo country fiddle under CC0 / CC BY; the
  Internet Archive's modern fiddle concerts (e.g. Darol Anger / Bruce Molsky / Brittany Haas,
  2009) are CC BY-NC-ND audience recordings with a band, not taken.
