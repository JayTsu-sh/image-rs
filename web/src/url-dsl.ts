// Frontend-side URL DSL serializer. Mirror of `src/domain/url_dsl.rs::parse`
// (in reverse). Used by the "Share as URL" panel — the resulting string can
// be appended to /v1/img/{key} as `?p=...`.

import type { Anchor, OpDto, OutputDto } from './types';

const ANCHOR_SHORT: Record<Anchor, string> = {
  top_left: 'tl',
  top: 't',
  top_right: 'tr',
  left: 'l',
  center: 'c',
  right: 'r',
  bottom_left: 'bl',
  bottom: 'b',
  bottom_right: 'br',
};

export function buildUrlDsl(ops: OpDto[], output: OutputDto): string {
  const segments: string[] = [];

  for (const op of ops) {
    switch (op.op) {
      case 'resize': {
        const parts = ['resize'];
        if (op.width !== undefined) parts.push(`w_${op.width}`);
        if (op.height !== undefined) parts.push(`h_${op.height}`);
        if (op.mode) parts.push(`m_${op.mode}`);
        if (op.interpolation && op.interpolation !== 'auto') {
          parts.push(`i_${op.interpolation}`);
        }
        segments.push(parts.join(','));
        break;
      }
      case 'rotate': {
        const parts = ['rotate', `a_${op.angle}`];
        if (op.background) parts.push(`bg_${op.background.replace('#', '')}`);
        segments.push(parts.join(','));
        break;
      }
      case 'crop':
        segments.push(
          `crop,x_${op.x},y_${op.y},w_${op.width},h_${op.height}`,
        );
        break;
      case 'blur':
        segments.push(`blur,s_${op.sigma}`);
        break;
      case 'sharpen': {
        const parts = ['sharpen', `a_${op.amount}`];
        if (op.radius !== undefined) parts.push(`r_${op.radius}`);
        segments.push(parts.join(','));
        break;
      }
      case 'round_corner':
        segments.push(`round,r_${op.radius}`);
        break;
      case 'brightness':
        segments.push(`brightness,v_${op.value}`);
        break;
      case 'contrast':
        segments.push(`contrast,v_${op.value}`);
        break;
      case 'saturation':
        segments.push(`saturation,f_${op.factor}`);
        break;
      case 'temperature':
        segments.push(`temperature,v_${op.value}`);
        break;
      case 'auto_orient':
        segments.push('auto_orient');
        break;
      case 'watermark_image':
        // Image watermark cannot be expressed in the URL — its asset is a
        // binary blob that must travel as multipart. Skip silently;
        // `hasImageWatermark()` lets the UI surface a warning instead.
        break;
      case 'watermark_text': {
        const parts = ['text', `t_${encodeURIComponent(op.text)}`];
        if (op.font) parts.push(`f_${op.font}`);
        if (op.size !== undefined) parts.push(`s_${op.size}`);
        if (op.color) parts.push(`c_${op.color.replace('#', '')}`);
        if (op.position) parts.push(`p_${ANCHOR_SHORT[op.position]}`);
        if (op.margin !== undefined) parts.push(`m_${op.margin}`);
        if (op.shadow) parts.push('sh_1');
        segments.push(parts.join(','));
        break;
      }
    }
  }

  // Output format / quality / lossless / progressive
  const fmtParts: string[] = [];
  if (output.format) fmtParts.push(`f_${output.format}`);
  if (output.lossless) {
    fmtParts.push('l_1');
  } else if (output.quality !== undefined && output.quality !== 82) {
    fmtParts.push(`q_${output.quality}`);
  }
  if (output.progressive) fmtParts.push('p_1');
  if (fmtParts.length > 0) {
    segments.push(`format,${fmtParts.join(',')}`);
  }

  return segments.join('/');
}

export function hasImageWatermark(ops: OpDto[]): boolean {
  return ops.some((op) => op.op === 'watermark_image');
}
