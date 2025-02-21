<script lang="ts">
	import { Tag, X } from "phosphor-svelte";
	import Button from "./button.svelte";
	import TextInput from "./text-input.svelte";
	import { t } from "svelte-i18n";
	import type { Snippet } from "svelte";

	const LIMIT = 6;
	const MIN_TAG_SIZE = 3;
	const MAX_TAG_SIZE = 30;
	const SEPARATORS = ["Enter", " ", ","];

	interface Props {
		tags?: string[];
		children?: Snippet;
	}

	let { tags = $bindable(["lorem", "ipsum"]), children }: Props = $props();

	function removeTag(i: number) {
		tags.splice(i, 1);
		tags = [...tags];
	}

	let tagInput: string | undefined = $state();

	function onTagInput(e: KeyboardEvent) {
		// Prevents the user from accidentally creating the emote when having the tags input selected
		// Also prevents them from typing a separator
		if (SEPARATORS.includes(e.key)) {
			e.preventDefault();
		}

		if (tagInput === undefined) {
			return;
		}

		// Removes spaces and commas in case they paste separators (Maybe remove all invalid characters)
		const sanitizedTagInput = tagInput.replace(/[\s,]+/g, "");

		if (
			SEPARATORS.includes(e.key) &&
			sanitizedTagInput.length >= MIN_TAG_SIZE &&
			sanitizedTagInput.length <= MAX_TAG_SIZE &&
			tags.length < LIMIT &&
			!tags.includes(sanitizedTagInput)
		) {
			tags = [sanitizedTagInput, ...tags];
			tagInput = "";
		}
	}
</script>

<TextInput
	disabled={tags.length >= LIMIT}
	placeholder={tags.length < LIMIT
		? $t("labels.enter_tags")
		: $t("labels.tag_limit_reached", { values: { limit: LIMIT } })}
	bind:value={tagInput}
	onkeypress={onTagInput}
>
	{@render children?.()}
	{#snippet icon()}
		<Tag />
	{/snippet}
</TextInput>
{#if tags && tags.length > 0}
	<div class="tags">
		{#each tags as tag, i}
			<Button secondary onclick={() => removeTag(i)}>
				<span>{tag}</span>
				<X size={1 * 16} />
			</Button>
		{/each}
	</div>
{/if}

<style lang="scss">
	.tags {
		margin-top: 0.25rem;

		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;

		& > :global(.button) {
			font-size: 0.875rem;
			padding: 0.22rem 0.5rem 0.22rem 0.62rem;
			font-weight: 500;
			max-width: 100%;
		}

		& > :global(.button > span) {
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}
</style>
