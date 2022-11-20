## GeoDB Server

A HTTP API for [geodb](https://github.com/daimaou92/geodb)

### How to Run

1. Create a new account with
[Maxmind for Geolite2](https://www.maxmind.com/en/geolite2/signup?utm_source=kb&utm_medium=kb-link&utm_campaign=kb-create-account)
2. Generate a Maxmind License key as instructed [here](https://support.maxmind.com/hc/en-us/articles/4407111582235-Generate-a-License-Key).
3. Export the generated Key:
```bash
export MAXMIND_KEY="abcdefghijklmnop"
```
4. Databases are as defined [here](https://github.com/daimaou92/geodb)
5. Determine a path at which these databases are going to be downloaded.
This app will require approx 85MiB of free space for DBs only
6. Export above path:
```bash
export GL2_DBDIR="/tmp/dbdir"
```
7. This application can be run with or without auth.
8. If auth is needed continue else jump to step 11
9. Create a file with one secret key in each line. Something like this:
```text
CBOI7AIHQcAKhaHVol
7toCW4ndOvNy0ssSot
L44eRv07k2KwndvLQm
EGwSQtwsPjzv1IO75Z
h2UvAAs63ZjhYnISjn
```
10. Now export above file:
```bash
export GEODB_AUTH_FILE="/tmp/geodb-server-auth.txt"
```
Any of the keys, one per line, from this file can be used for auth

11. Test the API (more in the API section below) out for an IP:
#### Authorized:
```bash
curl -H "Authorization:CBOI7AIHQcAKhaHVol" http://localhost:40000/country/ip/1.32.128.0
```

#### Unauthorized
```bash
curl http://localhost:40000/country/ip/1.32.128.0
```

### How to Build

1. This application uses protobufs and responds in protobufs only. The proto files are
available in `src/protos`. Naturally to build those you'll need to install
[protoc](https://developers.google.com/protocol-buffers/docs/downloads).
2. Once it's available in `PATH` - just do `cargo build` or `cargo build --release`


### APIs

This application has 4 APIs. All of them are GET HTTP Requests and each returns one
of the protobuf `Message`s defined in `src/protos/geo.proto`.

1. [GET] /country/ip/:ip
Given some IP(v4|v6) this responds with the proto message `Country`.
Responds with a 400 Bad Request in case no mapping is found

2. [GET] /country/iso/:iso
Given the ISO2 code, as defined in [ISO 3166](https://www.iso.org/iso-3166-country-codes.html),
of any country this route also responds with the proto message `Country`.
400 Bad Request if the code is not found.

3. [GET] /city/:ip
Given some IP(v4|v6) this responds with the proto message `City`.
400 Bad Request if not found.

4. [GET] /asn/:ip
Given some IP(v4|v6) this responds with the proto message `ASN`.
400 Bad Request if not found.

