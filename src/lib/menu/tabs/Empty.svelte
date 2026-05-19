<script lang="ts">
    export let name: string;
</script>

<div class="empty">
    <span class="title">{name}</span>
    <span class="hint">not implemented yet</span>
</div>

<style>
    .empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 4px;
        height: 100%;
        min-height: 220px;
        color: var(--text-muted);
    }
    .title {
        font-size: 12px;
        font-weight: 600;
        color: var(--text);
        opacity: 0.6;
    }
    .hint {
        font-size: 10px;
    }
</style>
