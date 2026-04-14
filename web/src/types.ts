// TypeScript mirror of the backend DTOs in src/interfaces/http/dto.rs.
// Keep in sync — these types are the contract between frontend and backend.

export type Anchor =
  | 'top_left'
  | 'top'
  | 'top_right'
  | 'left'
  | 'center'
  | 'right'
  | 'bottom_left'
  | 'bottom'
  | 'bottom_right';

export type ResizeMode = 'fit' | 'fill' | 'exact';

export type Interpolation =
  | 'auto'
  | 'nearest'
  | 'linear'
  | 'cubic'
  | 'area'
  | 'lanczos4';

export type ImageFormat = 'jpeg' | 'png' | 'webp';

export type DiffMode = 'highlight' | 'abs' | 'grayscale';

// Discriminated union — `op` is the tag matching backend's #[serde(tag = "op")]
export type OpDto =
  | {
      op: 'resize';
      width?: number;
      height?: number;
      mode?: ResizeMode;
      interpolation?: Interpolation;
    }
  | { op: 'rotate'; angle: number; background?: string }
  | { op: 'crop'; x: number; y: number; width: number; height: number }
  | { op: 'blur'; sigma: number }
  | { op: 'sharpen'; amount: number; radius?: number }
  | { op: 'round_corner'; radius: number }
  | { op: 'brightness'; value: number }
  | { op: 'contrast'; value: number }
  | { op: 'saturation'; factor: number }
  | { op: 'temperature'; value: number }
  | { op: 'auto_orient' }
  | {
      op: 'watermark_image';
      asset: string;
      position?: Anchor;
      opacity?: number;
      margin?: number;
      scale?: number;
    }
  | {
      op: 'watermark_text';
      text: string;
      font?: string;
      size?: number;
      color?: string;
      position?: Anchor;
      margin?: number;
      shadow?: boolean;
    };

export type OpKind = OpDto['op'];

export interface OutputDto {
  format?: ImageFormat;
  quality?: number;
  lossless?: boolean;
  progressive?: boolean;
}

export interface ProcessMeta {
  width: number;
  height: number;
  bytes: number;
  elapsedMs: number;
  cache?: 'hit' | 'miss';
}

export interface ProcessResult {
  blob: Blob;
  meta: ProcessMeta;
}

// Display metadata for the "+ Add operation" popover and op cards
export interface OpDescriptor {
  kind: OpKind;
  label: string;
  group: 'basic' | 'effect' | 'color' | 'watermark';
  icon: string; // Material Symbol name
}

export const OP_DESCRIPTORS: OpDescriptor[] = [
  { kind: 'resize',          label: 'Resize',           group: 'basic',     icon: 'aspect_ratio' },
  { kind: 'rotate',          label: 'Rotate',           group: 'basic',     icon: 'rotate_right' },
  { kind: 'crop',            label: 'Crop',             group: 'basic',     icon: 'crop' },
  { kind: 'blur',            label: 'Blur',             group: 'effect',    icon: 'blur_on' },
  { kind: 'sharpen',         label: 'Sharpen',          group: 'effect',    icon: 'deblur' },
  { kind: 'round_corner',    label: 'Round corner',     group: 'effect',    icon: 'rounded_corner' },
  { kind: 'brightness',      label: 'Brightness',       group: 'effect',    icon: 'wb_sunny' },
  { kind: 'contrast',        label: 'Contrast',         group: 'effect',    icon: 'contrast' },
  { kind: 'auto_orient',     label: 'Auto-orient',      group: 'effect',    icon: 'screen_rotation' },
  { kind: 'saturation',      label: 'Saturation',       group: 'color',     icon: 'palette' },
  { kind: 'temperature',     label: 'Temperature',      group: 'color',     icon: 'thermostat' },
  { kind: 'watermark_image', label: 'Image watermark',  group: 'watermark', icon: 'branding_watermark' },
  { kind: 'watermark_text',  label: 'Text watermark',   group: 'watermark', icon: 'title' },
];

/// Build a default OpDto for a given kind. The store calls this when the
/// user clicks "+ Add operation > X" — the resulting card is then editable.
export function defaultOp(kind: OpKind): OpDto {
  switch (kind) {
    case 'resize':          return { op: 'resize', width: 800, mode: 'fit', interpolation: 'auto' };
    case 'rotate':          return { op: 'rotate', angle: 0 };
    case 'crop':            return { op: 'crop', x: 0, y: 0, width: 100, height: 100 };
    case 'blur':            return { op: 'blur', sigma: 2.0 };
    case 'sharpen':         return { op: 'sharpen', amount: 0.5, radius: 1.0 };
    case 'round_corner':    return { op: 'round_corner', radius: 16 };
    case 'brightness':      return { op: 'brightness', value: 0 };
    case 'contrast':        return { op: 'contrast', value: 1.0 };
    case 'saturation':      return { op: 'saturation', factor: 1.0 };
    case 'temperature':     return { op: 'temperature', value: 0 };
    case 'auto_orient':     return { op: 'auto_orient' };
    case 'watermark_image': return { op: 'watermark_image', asset: 'watermark', position: 'bottom_right', opacity: 0.7, margin: 16, scale: 0.2 };
    case 'watermark_text':  return { op: 'watermark_text', text: '© 2026', size: 24, color: '#ffffffff', position: 'bottom_right', margin: 16, shadow: false };
  }
}
