<script setup lang="ts">
import AppHeader from './components/AppHeader.vue';
import SourcePicker from './components/SourcePicker.vue';
import OperationList from './components/OperationList.vue';
import PreviewPane from './components/PreviewPane.vue';
import OutputPanel from './components/OutputPanel.vue';
</script>

<template>
  <div class="h-full flex flex-col">
    <AppHeader />
    <main class="flex flex-1 overflow-hidden">
      <!-- LEFT: source + operations list + add button -->
      <aside
        class="w-[320px] bg-surface-container-low flex flex-col border-r border-outline-variant/20"
      >
        <SourcePicker />
        <OperationList />
      </aside>

      <!-- CENTER: preview pane -->
      <PreviewPane />

      <!-- RIGHT: output settings -->
      <OutputPanel />
    </main>
  </div>
</template>
