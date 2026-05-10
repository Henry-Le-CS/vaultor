<script lang="ts">
  import DoorScreen from './lib/components/login/DoorScreen.svelte';
  import AppShell from './lib/components/layout/AppShell.svelte';
</script>

<!-- AppShell always lives in the DOM; DoorScreen overlays it -->
<AppShell />
<DoorScreen />
