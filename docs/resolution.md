# Resolution: why 720 dpi RGB is enough

The SC-P900 is advertised with up to **5760 × 1440 dpi**, yet this driver – like
Epson's own Windows and macOS drivers – sends the image at **720 × 720 dpi**.
That is not a limitation. The two numbers describe different things.

## Image resolution vs. printer resolution

| | Image resolution (what is sent) | Printer resolution (what is advertised) |
|---|---|---|
| Value | 720 × 720 | up to 5760 × 1440 |
| What one dot can be | a **color value**: RGB with 256 levels per channel | **a droplet or nothing**: per ink no drop, small, medium or large |
| Unit, strictly speaking | pixels per inch (ppi) | droplet positions per inch (dpi) |

An inkjet printer cannot print a dot that is "37 % cyan". It can only decide
whether a droplet lands at a position and how big it is. Intermediate tones are
produced by spreading many droplets in a fine, irregular pattern
(**halftoning / dithering**). On the SC-P900 the firmware does this.

## The numbers

At 5760 × 1440 dpi there are **8 × 2 = 16 droplet positions per ink** for every
720 ppi image pixel, each with four states (none, small, medium, large). In
addition the printer has light inks (light cyan, vivid light magenta, gray,
light gray), so light tones can be built from more, less visible droplets.

The high printer resolution is therefore not used for more image detail, but
for **finer tonal steps and an invisible halftone pattern**.

## Multiple passes

The nozzles on the print head are physically much further apart than
1/1440 inch. The vertical 1440 dpi is achieved by passing the head over the
paper several times with a small paper advance in between, so that the rows of
different passes interleave (Epson calls the scheme *MicroWeave*). Each line is
printed by several different nozzles, which also hides banding from individual
weak nozzles. Higher quality settings mean more passes – and slower printing.

## Evidence from the official driver

A capture of the Windows driver with quality set to **5760 dpi** and
"No Color Adjustment" shows that it still sends **720 × 720 dpi RGB** data.
The quality setting only changes a byte in the job header that tells the
firmware how to place the droplets (passes, halftone).

## Practical advice

- About **300 ppi** at the final print size is enough for photographs viewed at
  normal distance; the eye cannot resolve much more at ~30 cm.
- Higher input resolution (up to 720 ppi) can help a little with very fine lines
  and small text.
- Anything above 720 ppi is reduced to 720 ppi before it reaches the printer and
  gains nothing.
- If you care about sharpening, resample the image to the final print size in a
  good image editor yourself. (When the command-line tool is given a print size
  with `--image-width-mm`, it scales with Lanczos resampling.)

## What about Gutenprint?

Driving the printer with Gutenprint (raw ESC/P2 nozzle data) does **not** give
more image detail either – nobody can see more than ~720 ppi. Its potential
advantage is control over the halftoning itself: droplet sizes, how light and
dark inks are mixed, ink limits per paper and an own calibration. Whether that
can beat Epson's firmware halftoning is an open question.
