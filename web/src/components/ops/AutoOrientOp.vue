<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import type { OpEntry } from '../../stores/pipeline';

defineProps<{ entry: OpEntry }>();
</script>

<template>
  <OperationCard :entry="entry" icon="screen_rotation" title="Auto-orient">
    <p class="text-xs text-on-surface-variant">
      Reads EXIF orientation and rotates the image accordingly. No parameters.
    </p>
  </OperationCard>
</template>
