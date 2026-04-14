<script setup lang="ts">
const props = defineProps<{ label: string; modelValue: string }>();
const emit = defineEmits<{ (e: 'update:modelValue', v: string): void }>();

// Color picker element takes #rrggbb (no alpha). We preserve a separate
// alpha byte so user-set #rrggbbaa strings round-trip correctly.
function baseHex(): string {
  const s = props.modelValue || '#ffffff';
  return s.length >= 7 ? s.slice(0, 7) : s;
}
function alphaHex(): string {
  const s = props.modelValue || '#ffffffff';
  return s.length === 9 ? s.slice(7, 9) : 'ff';
}

function onPicker(e: Event) {
  const base = (e.target as HTMLInputElement).value;
  emit('update:modelValue', base + alphaHex());
}
function onText(e: Event) {
  emit('update:modelValue', (e.target as HTMLInputElement).value);
}
</script>

<template>
  <div class="space-y-1">
    <label
      class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
    >
      {{ label }}
    </label>
    <div class="flex items-center gap-2">
      <input
        type="color"
        :value="baseHex()"
        class="h-8 w-12 rounded cursor-pointer border border-outline-variant/20"
        @input="onPicker"
      />
      <input
        type="text"
        :value="modelValue"
        class="flex-1 bg-surface-container-low border-0 rounded-md px-3 py-1.5 text-xs font-mono focus:ring-1 focus:ring-primary focus:outline-none"
        @input="onText"
      />
    </div>
  </div>
</template>
