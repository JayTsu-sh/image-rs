<script setup lang="ts">
import { ref } from 'vue';
import { onClickOutside } from '@vueuse/core';
import { OP_DESCRIPTORS, defaultOp } from '../types';
import type { OpKind } from '../types';
import { usePipeline } from '../stores/pipeline';

const pipeline = usePipeline();
const open = ref(false);
const containerRef = ref<HTMLElement | null>(null);

onClickOutside(containerRef, () => {
  open.value = false;
});

function selectOp(kind: OpKind) {
  pipeline.addOp(defaultOp(kind));
  open.value = false;
}

const GROUPS = [
  { title: 'Basic', ops: OP_DESCRIPTORS.filter((d) => d.group === 'basic') },
  { title: 'Effect', ops: OP_DESCRIPTORS.filter((d) => d.group === 'effect') },
  { title: 'Color', ops: OP_DESCRIPTORS.filter((d) => d.group === 'color') },
  {
    title: 'Watermark',
    ops: OP_DESCRIPTORS.filter((d) => d.group === 'watermark'),
  },
];
</script>

<template>
  <div ref="containerRef" class="relative">
    <button
      class="w-full flex items-center justify-center gap-2 bg-surface-container-lowest text-on-surface shadow-md border border-outline-variant/20 py-3 rounded-xl font-semibold hover:shadow-lg transition-all"
      @click="open = !open"
    >
      <span class="material-symbols-outlined">{{ open ? 'close' : 'add' }}</span>
      Add Operation
    </button>

    <div
      v-if="open"
      class="absolute bottom-full left-0 right-0 mb-2 bg-surface-container-lowest rounded-xl shadow-xl border border-outline-variant/10 p-2 max-h-[60vh] overflow-y-auto z-50"
    >
      <div
        v-for="group in GROUPS"
        :key="group.title"
        class="mb-2 last:mb-0"
      >
        <div
          class="px-3 py-1 text-[10px] font-mono uppercase text-on-surface-variant/80 tracking-wider"
        >
          {{ group.title }}
        </div>
        <button
          v-for="op in group.ops"
          :key="op.kind"
          class="w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm text-on-surface hover:bg-surface-container transition-colors"
          @click="selectOp(op.kind)"
        >
          <span
            class="material-symbols-outlined text-[18px] text-on-surface-variant"
          >
            {{ op.icon }}
          </span>
          <span>{{ op.label }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
