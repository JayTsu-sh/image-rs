# Design System Document: The Precision Editor

## 1. Overview & Creative North Star
**Creative North Star: "The Technical Atelier"**

This design system moves beyond the generic "SaaS dashboard" aesthetic to create a space that feels like a high-end photography studio—clinical, precise, yet undeniably premium. While the user request calls for a "tool-like" aesthetic, we achieve this not through clutter, but through **Atmospheric Precision**. 

We reject the "boxed-in" look of traditional GUIs. Instead of heavy borders and rigid grids, we utilize **Tonal Layering** and **Intentional Asymmetry**. By prioritizing white space as a functional element rather than "empty" space, we guide the user's eye toward the image-processing core, making the interface feel like an extension of the creative process itself.

---

## 2. Colors & Surface Architecture
The palette is rooted in a pristine white foundation, using Indigo not just as a brand mark, but as a "signal" for interaction and technical accuracy.

### The "No-Line" Rule
Traditional 1px solid borders are strictly prohibited for sectioning. Structural boundaries must be defined through background color shifts. 
- Use `surface-container-low` (#f1f4f6) to define the sidebar or utility panels against the `surface` (#f8f9fa) canvas. 
- This creates a seamless "monolithic" look that feels carved from a single block rather than assembled from parts.

### Surface Hierarchy & Nesting
Depth is achieved through the physical stacking of tones. 
- **Base Layer:** `surface` (#f8f9fa)
- **Content Sections:** `surface-container-low` (#f1f4f6)
- **Interactive Cards:** `surface-container-lowest` (#ffffff) sitting atop a `low` or `mid` container.
- **Nesting Logic:** An image preview window (High Priority) should sit in `surface-container-highest` (#dbe4e7) to create a subtle "inset" look, suggesting it is a deep workspace.

### The "Glass & Gradient" Rule
To add "soul" to the technical tool:
- **Floating Modals/Overlays:** Use `surface-container-lowest` at 85% opacity with a `backdrop-filter: blur(12px)`.
- **Primary CTAs:** Apply a subtle linear gradient from `primary` (#4d44e3) to `primary_dim` (#4034d7) at a 135-degree angle. This prevents the indigo from looking "flat" and adds a tactile, pressed-ink quality.

---

### 3. Typography: Editorial Utility
We use **Inter** as a variable font to bridge the gap between technical data and high-end editorial design.

| Level | Token | Size | Weight | Intent |
| :--- | :--- | :--- | :--- | :--- |
| **Display** | `display-md` | 2.75rem | 600 (Semi-Bold) | Hero stats or high-level status. |
| **Headline** | `headline-sm` | 1.5rem | 500 (Medium) | Section headers; use tight letter-spacing (-0.02em). |
| **Title** | `title-sm` | 1.0rem | 600 | Card titles and tool categories. |
| **Body** | `body-md` | 0.875rem | 400 (Regular) | Primary tool descriptions and labels. |
| **Technical** | `label-sm` | 0.6875rem | 500 (Mono) | Metadata, EXIF data, and coordinate badges. |

**The Typography Philosophy:** Use `label-sm` in a monospace variant for all technical "output" data (e.g., "ISO 400", "F/2.8"). This distinguishes *system data* from *user interface* text.

---

## 4. Elevation & Depth
In "The Technical Atelier," we avoid heavy shadows. We use **Tonal Layering** to convey importance.

- **The Layering Principle:** Place a `surface-container-lowest` card (Pure White) on a `surface-container-low` background. The 1% shift in value is enough for the human eye to perceive elevation without the clutter of a shadow.
- **Ambient Shadows:** Only for floating elements (e.g., right-click menus). 
  - `box-shadow: 0 12px 32px -4px rgba(77, 68, 227, 0.04);` (A tinted shadow using the Primary color at very low opacity).
- **The "Ghost Border":** If a border is required for accessibility (e.g., input fields), use `outline-variant` (#abb3b7) at **20% opacity**. It should be felt, not seen.

---

## 5. Components

### Buttons
- **Primary:** Gradient fill (`primary` to `primary_dim`), `on_primary` text, `lg` (0.5rem) roundedness.
- **Secondary:** `surface-container-high` background, `on_surface` text. No border.
- **Tertiary (Ghost):** No background. `primary` text. Use for low-emphasis actions like "Cancel."

### Cards & Lists
- **Rule:** Absolute prohibition of divider lines. 
- **Separation:** Use `16px` gutters and vertical whitespace. For lists, use a hover state change to `surface-container-high` to define rows.
- **Padding:** Standardize on `12px` internal padding for "Tool" cards to maintain a compact, professional density.

### Technical Badges (Technical Data)
- **Style:** Use `surface-variant` background with `on_surface_variant` text. 
- **Typography:** `label-sm` (Monospace).
- **Roundedness:** `sm` (0.125rem) to maintain a "sharp" professional feel.

### Input Fields (Sliders & Toggles)
- **Sliders:** The track should be `surface-container-highest`. The active fill is `primary`. The handle (thumb) is `surface-container-lowest` with a 2px `primary` border.
- **Inputs:** `surface-container-low` background, no border, `md` roundedness. Focus state shifts background to `surface-container-lowest` with a 1px `primary` ghost border.

---

## 6. Do's and Don'ts

### Do
- **Do** use `primary-fixed-dim` for inactive toggle states to maintain a tonal connection to the brand.
- **Do** prioritize "Breathing Room." If a layout feels cramped, increase the white space instead of adding a border.
- **Do** align technical data badges to a strict grid to reinforce the "tool" aesthetic.

### Don't
- **Don't** use pure black (#000000) for text. Always use `on_surface` (#2b3437) to maintain the soft-minimalist integrity.
- **Don't** use standard "Drop Shadows." They break the clinical, flat-layered aesthetic of the Atelier.
- **Don't** use icons with varying stroke weights. Stick to Phosphor/Lucide "Regular" (approx 1.5px to 2px) to match the `body-md` font weight.

---

## 7. Signature Interaction: The "Focus Shift"
When a user selects an image or a tool, the rest of the interface should subtly "recede." This is achieved by shifting the non-active `surface-container` elements to a slightly lower opacity (80%) while keeping the active element at 100%. This creates a "spotlight" effect without using intrusive overlays.