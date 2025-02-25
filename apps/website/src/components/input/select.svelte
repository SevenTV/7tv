<script lang="ts" module>
	export type Option = {
		value: string;
		label: string;
		icon?: Snippet;
	};
</script>

<script lang="ts">
	import mouseTrap from "$/lib/mouseTrap";
	import { CaretDown } from "phosphor-svelte";
	import { fade } from "svelte/transition";
	import Button from "./button.svelte";
	import type { Snippet } from "svelte";
	import { t } from "svelte-i18n";
	import type { HTMLAttributes } from "svelte/elements";

	type Props = {
		options: Option[];
		selected?: string;
		grow?: boolean;
		disabled?: boolean;
	} & HTMLAttributes<HTMLDivElement>;

	let {
		options,
		selected = $bindable(options[0]?.value ?? undefined),
		grow = false,
		disabled = false,
		...restProps
	}: Props = $props();

	let selectedLabel = $derived(options.find((o) => o.value === selected));

	let expanded = $state(false);

	function toggle() {
		expanded = !expanded;
	}

	function close() {
		expanded = false;
	}

	function select(option: string) {
		selected = option;
		expanded = false;
	}

	let width = $state<number>();
</script>

<div
	use:mouseTrap={close}
	class="select"
	class:grow
	class:expanded
	{...restProps}
	style={expanded ? `min-width: ${width}px; ${restProps.style}` : restProps.style}
>
	<select bind:value={selected} onclick={toggle} onkeypress={toggle} {disabled}>
		{#each options as option}
			<option value={option.value}>
				{option.value}
			</option>
		{/each}
	</select>
	<Button secondary tabindex={-1} onclick={toggle} {disabled}>
		{#if selectedLabel}
			{@render selectedLabel.icon?.()}
			{selectedLabel.label}
		{:else}
			{$t("labels.select")}
		{/if}
		{#snippet iconRight()}
			<CaretDown size={1 * 16} />
		{/snippet}
	</Button>
	{#if expanded}
		<div class="dropped" transition:fade={{ duration: 100 }} bind:offsetWidth={width}>
			{#each options as option}
				<Button onclick={() => select(option.value)}>
					{@render option.icon?.()}
					{option.label}
				</Button>
			{/each}
		</div>
	{/if}
</div>

<style lang="scss">
	select {
		-webkit-appearance: none;
		-moz-appearance: none;
		appearance: none;
		outline: none;
		margin: 0;
		padding: 0;
		border: none;
		width: 0;

		display: inline;
		clip: rect(0 0 0 0);
		clip-path: inset(50%);
		height: 1px;
		overflow: hidden;
		position: absolute;
		white-space: nowrap;
		width: 1px;

		&:focus-visible + :global(.button) {
			border-color: var(--primary);
		}
	}

	.select {
		position: relative;

		&.grow {
			width: 100%;
			flex-grow: 1;
		}

		& > :global(.button) {
			width: 100%;
			justify-content: space-between;
		}

		&.expanded {
			& > :global(.button) {
				border-color: var(--border-active);
				border-bottom-left-radius: 0;
				border-bottom-right-radius: 0;
			}

			& > .dropped {
				border-top-left-radius: 0;
				border-top-right-radius: 0;
			}
		}
	}

	.dropped {
		z-index: 1;

		position: absolute;
		top: 100%;
		right: 0;
		margin: 0;
		padding: 0;
		border: var(--border-active) 1px solid;
		border-top: none;
		border-radius: 0.5rem;

		min-width: 100%;
		max-height: 15rem;
		overflow: auto;
		scrollbar-gutter: stable;

		background-color: var(--bg-medium);
		box-shadow: 4px 4px 8px rgba(0, 0, 0, 0.1);

		& > :global(.button) {
			border-radius: 0;
			width: 100%;
			padding: 0.5rem 1rem;

			&:last-child {
				border-bottom-left-radius: 0.5rem;
				border-bottom-right-radius: 0.5rem;
			}
		}
	}
</style>
