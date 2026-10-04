# Security policy

Colophon runs offline: no server, no account, no telemetry, ever. The
application makes exactly one network call, and only while you leave it on.
At launch it asks GitHub whether a newer version exists. What goes out is an
HTTPS request to the releases feed; what GitHub sees of it is the IP address
and user agent of that request, nothing of your photographs, nothing of your
album, and no identifier Colophon invents. Nothing is downloaded or installed
without a click. Preferences (Cmd-comma) turns the check off, and then
nothing leaves the machine at all.

Two things you can check rather than take on trust. The interface cannot
reach the network whatever it tries: its content security policy allows
`connect-src 'self' ipc:` and nothing else, so that one call is made by the
updater in the Rust process and nowhere else. And reporting a bug opens a
prefilled GitHub page in your browser, on your click; the application uploads
nothing, which is why the same screen offers to copy the report instead.

The command line, built from source, can reach one more host, and only when
you hand it your own key. Its hidden `--devis` flag asks Cloudprinter's API
(`api.cloudprinter.com`) for a quote: what goes out is the key, read from the
`CLOUDPRINTER_SANDBOX_API_KEY` environment variable, the product and paper
codes, the page count, the number of copies and the delivery country, nothing
of your photographs. For that, the engine carries an HTTP client, `ureq`,
which the application never calls. The engine also holds code that places the
two PDFs on an S3 bucket you name and orders from Cloudprinter; no flag of
the command line runs it, only a test you start by hand with your own keys.

The attack surface is the files it reads (your images, `album.json`) and the
files it writes (PDF, thumbnails). A crafted image or album file that crashes
the app is a bug; one that executes code or reads files outside the album
folder is a security problem, and so is anything that makes a photograph or a
path leave the machine.

Report security problems privately through
[GitHub security advisories](https://github.com/alexis-morain/colophon/security/advisories/new),
not in a public issue. You will get an answer within 48 hours and a fix as
fast as one person can write it, in the next release. There is no bug bounty:
this is free software maintained by one person, and the reward on offer is a
prompt fix and your name in the release notes if you want it there.
