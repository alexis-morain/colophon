# Security policy

Colophon runs entirely offline: no server, no account, no telemetry, no
network call at runtime. The attack surface is the files it reads (your
images, `album.json`) and the files it writes (PDF, thumbnails). A crafted
image or album file that crashes the app is a bug; one that executes code or
reads files outside the album folder is a security problem, and so is
anything that makes a photograph or a path leave the machine.

Report security problems privately through
[GitHub security advisories](https://github.com/alexis-morain/colophon/security/advisories/new),
not in a public issue. You will get an answer within 48 hours and a fix as
fast as one person can write it, in the next release. There is no bug bounty:
this is free software maintained by one person, and the reward on offer is a
prompt fix and your name in the release notes if you want it there.
