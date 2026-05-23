<script lang="ts" context="module">
    export type Node = { name: string; address: string; children: Node[] };
</script>

<script lang="ts">
    import { getAvatar } from "../avatar";
    export let node: Node;
    export let depth: number = 0;
    export let parentName: string = "";
    // Root opens by default, deeper levels stay collapsed.
    let open = depth === 0;
    $: hasChildren = node.children && node.children.length > 0;
    // A node is treated as a player when its parent is the Players service.
    // That's the only place in a Roblox DataModel where a node's name is a
    // username we can resolve to an avatar.
    $: isPlayer = parentName === "Players" && !!node.name;

    let avatarUrl: string | null = null;
    $: if (isPlayer) {
        getAvatar(node.name).then(u => { avatarUrl = u; });
    } else {
        avatarUrl = null;
    }

    function copyAddr(e: MouseEvent) {
        e.stopPropagation();
        navigator.clipboard?.writeText(node.address).catch(() => {});
    }
</script>

<div class="node">
    <button class="row" on:click={() => (open = !open)} style="padding-left: {depth * 10 + 4}px;">
        {#if hasChildren}
            <span class="caret" class:open>▸</span>
        {:else}
            <span class="caret hidden">▸</span>
        {/if}
        {#if isPlayer}
            <span class="avatar">
                {#if avatarUrl}
                    <img src={avatarUrl} alt="" draggable="false" />
                {/if}
            </span>
        {/if}
        <span class="name">{node.name || "(unnamed)"}</span>
        <span class="addr" on:click={copyAddr} title="click to copy">{node.address}</span>
    </button>
    {#if open && hasChildren}
        <div class="children">
            {#each node.children as child}
                <svelte:self node={child} depth={depth + 1} parentName={node.name} />
            {/each}
        </div>
    {/if}
</div>

<style>
    .node {
        display: flex;
        flex-direction: column;
    }
    .row {
        display: flex;
        align-items: center;
        gap: 5px;
        background: transparent;
        border: 0;
        font: inherit;
        color: var(--text);
        font-size: 10px;
        padding: 2px 4px 2px 0;
        cursor: pointer;
        text-align: left;
        border-radius: 3px;
    }
    .row:hover { background: var(--bg-input); }
    .caret {
        width: 10px;
        font-size: 8px;
        color: var(--text-muted);
        transition: transform 120ms var(--ease);
        display: inline-block;
        text-align: center;
    }
    .caret.open { transform: rotate(90deg); }
    .caret.hidden { visibility: hidden; }
    .avatar {
        width: 14px;
        height: 14px;
        border-radius: 50%;
        background: var(--bg-input);
        border: 1px solid var(--border);
        overflow: hidden;
        flex-shrink: 0;
        display: inline-flex;
    }
    .avatar img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }
    .name {
        flex: 1;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .addr {
        font-family: ui-monospace, Menlo, monospace;
        font-size: 9px;
        color: var(--text-muted);
        padding: 0 4px;
        border-radius: 3px;
    }
    .addr:hover { background: var(--accent); color: #000; }
    .children {
        border-left: 1px dashed var(--border-soft);
        margin-left: 12px;
    }
</style>
