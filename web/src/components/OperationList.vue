<script setup lang="ts">
import { VueDraggable } from 'vue-draggable-plus';
import { usePipeline } from '../stores/pipeline';
import AddOperationPopover from './AddOperationPopover.vue';
import ResizeOp from './ops/ResizeOp.vue';
import RotateOp from './ops/RotateOp.vue';
import CropOp from './ops/CropOp.vue';
import BlurOp from './ops/BlurOp.vue';
import SharpenOp from './ops/SharpenOp.vue';
import RoundCornerOp from './ops/RoundCornerOp.vue';
import BrightnessOp from './ops/BrightnessOp.vue';
import ContrastOp from './ops/ContrastOp.vue';
import SaturationOp from './ops/SaturationOp.vue';
import TemperatureOp from './ops/TemperatureOp.vue';
import AutoOrientOp from './ops/AutoOrientOp.vue';
import WatermarkImageOp from './ops/WatermarkImageOp.vue';
import WatermarkTextOp from './ops/WatermarkTextOp.vue';

const pipeline = usePipeline();
</script>

<template>
  <div class="flex-1 overflow-y-auto px-4 pb-4">
    <p
      v-if="pipeline.opCount === 0"
      class="text-xs text-on-surface-variant text-center py-12 px-4 leading-relaxed"
    >
      No operations yet. <br />
      <span class="text-on-surface-variant/70">
        Click "+ Add Operation" below to start building a pipeline.
      </span>
    </p>
    <VueDraggable
      v-model="pipeline.ops"
      handle=".drag-handle"
      :animation="150"
      class="space-y-3"
    >
      <template v-for="entry in pipeline.ops" :key="entry.id">
        <ResizeOp v-if="entry.data.op === 'resize'" :entry="entry" />
        <RotateOp v-else-if="entry.data.op === 'rotate'" :entry="entry" />
        <CropOp v-else-if="entry.data.op === 'crop'" :entry="entry" />
        <BlurOp v-else-if="entry.data.op === 'blur'" :entry="entry" />
        <SharpenOp v-else-if="entry.data.op === 'sharpen'" :entry="entry" />
        <RoundCornerOp
          v-else-if="entry.data.op === 'round_corner'"
          :entry="entry"
        />
        <BrightnessOp
          v-else-if="entry.data.op === 'brightness'"
          :entry="entry"
        />
        <ContrastOp v-else-if="entry.data.op === 'contrast'" :entry="entry" />
        <SaturationOp
          v-else-if="entry.data.op === 'saturation'"
          :entry="entry"
        />
        <TemperatureOp
          v-else-if="entry.data.op === 'temperature'"
          :entry="entry"
        />
        <AutoOrientOp
          v-else-if="entry.data.op === 'auto_orient'"
          :entry="entry"
        />
        <WatermarkImageOp
          v-else-if="entry.data.op === 'watermark_image'"
          :entry="entry"
        />
        <WatermarkTextOp
          v-else-if="entry.data.op === 'watermark_text'"
          :entry="entry"
        />
      </template>
    </VueDraggable>
  </div>
  <div class="p-4">
    <AddOperationPopover />
  </div>
</template>
