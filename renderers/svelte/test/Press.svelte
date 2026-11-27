<script>
  let { node, event } = $props();
</script>

<button class="mine" onclick={() => event("click")}>{node.inputs.label.text}</button>
