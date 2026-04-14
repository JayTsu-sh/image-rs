<script setup lang="ts">
import type { OpEntry } from '../stores/pipeline';
import { usePipeline } from '../stores/pipeline';

defineProps<{
  entry: OpEntry;
  icon: string;
  title: string;
  /// Optional small monospace badge in the title row (e.g. "LANCZOS4").
  badge?: string;
}>();

const pipeline = usePipeline();
</script>

<template>
  <div
    class="bg-surface-container-lowest rounded-lg border border-outline-variant/10 overflow-hidden"
  >
    <!-- Title row -->
    <div
      class="p-3 flex items-center justify-between bg-surface-container/40"
      @click="pipeline.toggleOp(entry.id)"
    >
      <div class="flex items-center gap-2 flex-1 min-w-0">
        <span
          class="material-symbols-outlined text-on-surface-variant cursor-grab drag-handle shrink-0"
          @click.stop
        >
          drag_indicator
        </span>
        <span
          class="material-symbols-outlined text-on-surface-variant text-[18px] shrink-0"
        >
          {{ icon }}
        </span>
        <span class="text-sm font-semibold truncate">{{ title }}</span>
      </div>
      <div class="flex items-center gap-1 shrink-0">
        <span
          v-if="badge"
          class="bg-surface-variant px-1.5 py-0.5 rounded-sm font-mono text-[10px] text-on-surface-variant uppercase tracking-wide"
        >
          {{ badge }}
        </span>
        <button
          class="p-1 hover:bg-surface-container-high rounded transition-colors"
          @click.stop="pipeline.toggleOp(entry.id)"
        >
          <span class="material-symbols-outlined text-sm">
            {{ entry.expanded ? 'keyboard_arrow_up' : 'keyboard_arrow_down' }}
          </span>
        </button>
        <button
          class="p-1 hover:bg-error-container hover:text-on-error-container rounded transition-colors"
          @click.stop="pipeline.removeOp(entry.id)"
        >
          <span class="material-symbols-outlined text-sm">close</span>
        </button>
      </div>
    </div>
    <!-- Body (params) -->
    <div v-if="entry.expanded" class="p-4 space-y-4">
      <slot />
    </div>
  </div>
</template>
