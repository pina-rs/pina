# Pina logo

Weave is a pineapple made from interlocking ribbons, with three leaves forming its crown.

<img src="./logo.png" alt="Pina: a pineapple made from interlocking ribbons" width="180">

## Files

| File                               | Use                                                               |
| ---------------------------------- | ----------------------------------------------------------------- |
| [logo.svg](./logo.svg)             | Gold vector with a transparent background.                        |
| [logo.png](./logo.png)             | Transparent 1024 × 1024 image for READMEs and package registries. |
| [avatar.png](./avatar.png)         | 1024 × 1024 image on ivory for the GitHub organisation avatar.    |
| [logo-mono.svg](./logo-mono.svg)   | Dark monochrome mark for light backgrounds.                       |
| [logo-dark.svg](./logo-dark.svg)   | Ivory mark for dark backgrounds.                                  |
| [logo.gif](./logo.gif)             | Four-second animation preview on ivory.                           |
| [logo.riv](./logo.riv)             | Rive animation with a transparent background.                     |
| [rive/scene.rml](./rive/scene.rml) | Editable Rive source.                                             |

Use gold `#E7A515`, ivory `#FAF8EF`, and dark green `#20251F`. Keep the square canvas and its clear space when placing the mark. READMEs use the static image; motion is available separately.

## Animation

The ribbons separate, slot back together, and hold while the crown opens. Load `logo.riv` with the `Logo` state machine. Its `Signature` timeline loops at 60 fps over four seconds. Show the static mark when reduced motion is requested.

[View the animated preview](./logo.gif).

The animation was built with Rive CLI 1.2.0. To rebuild it, run these commands from the repository root inside `devenv shell`:

```sh
rive .github/assets/rive --verify
rive .github/assets/rive --once
cp .github/assets/rive/build/pina-weave.riv .github/assets/logo.riv
```

The documentation uses a copy at `docs/src/assets/logo.png`, theme icons at `docs/theme/favicon.svg` and `docs/theme/favicon.png`, and a 64 × 64 export at `docs/src/favicon.png`. Both theme icons are needed because modern browsers prefer mdBook's SVG icon. The organisation README uses a copy at `profile/assets/logo.png` in [pina-rs/.github](https://github.com/pina-rs/.github). When changing the logo, update these copies and the organisation avatar alongside the canonical files here.
