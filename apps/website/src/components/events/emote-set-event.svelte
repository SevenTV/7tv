<script lang="ts">
	import type { EmoteSet, EmoteSetEvent, User, Emote } from "$/gql/graphql";
	import { FolderSimple, IconContext, Minus, PencilSimple, Plus } from "phosphor-svelte";
	import moment from "moment/min/moment-with-locales";
	import FromNow from "$/components/from-now.svelte";
	import ResponsiveImage from "../responsive-image.svelte";

	let { event }: { event: EmoteSetEvent } = $props();
</script>

{#snippet emoteSetLink(emoteSet?: EmoteSet | null)}
	{#if emoteSet}
		<a href="/emote-sets/{emoteSet.id}">
			<b>{emoteSet.name}</b>
		</a>
	{:else}
		<s>Deleted Set</s>
	{/if}
{/snippet}

{#snippet emoteLink(emote?: Emote | null)}
	{#if emote}
		<a href="/emotes/{emote.id}" class="emote-link" title={emote.defaultName}>
			<span class="emote-name">{emote.defaultName}</span>
			<ResponsiveImage images={emote.images} width={1.5 * 16} />
		</a>
	{:else}
		<s>Deleted Emote</s>
	{/if}
{/snippet}

{#snippet userLink(actor?: User | null, by: boolean = true)}
	{#if actor && actor.mainConnection}
		{#if by}
			by
		{/if}
		<a href="/users/{actor.id}" class="user-link" style:color={actor.highestRoleColor?.hex}>
			{actor.mainConnection.platformDisplayName}
		</a>
	{/if}
{/snippet}

<IconContext
	values={{
		style: "grid-area: icon; margin: 0 0.5rem;",
		size: 1.2 * 16,
		color: "var(--primary)",
	}}
>
	<div class="event">
		{#if event.data.__typename === "EventEmoteSetDataCreate"}
			<FolderSimple />
			<span class="text">
				{@render emoteSetLink(event.target)}
				created
				{@render userLink(event.actor)}
			</span>
		{:else if event.data.__typename === "EventEmoteSetDataChangeName"}
			<FolderSimple />
			<span class="text">
				{@render emoteSetLink(event.target)}
				renamed from {event.data.oldName} to {event.data.newName}
				{@render userLink(event.actor)}
			</span>
		{:else if event.data.__typename === "EventEmoteSetDataChangeTags"}
			<FolderSimple />
			<span class="text">
				Changed tags for
				{@render emoteSetLink(event.target)}
				from {event.data.oldTags} to {event.data.newTags}
				{@render userLink(event.actor)}
			</span>
		{:else if event.data.__typename === "EventEmoteSetDataChangeCapacity"}
			<FolderSimple />
			<span class="text">
				Changed capacity for
				{@render emoteSetLink(event.target)}
				from {event.data.oldCapacity} to {event.data.newCapacity}
				{@render userLink(event.actor)}
			</span>
		{:else if event.data.__typename === "EventEmoteSetDataAddEmote"}
			<Plus />
			<span class="text">
				Added emote
				{@render emoteLink(event.data.addedEmote)}
				to
				{@render emoteSetLink(event.target)}
				{@render userLink(event.actor)}
			</span>
		{:else if event.data.__typename === "EventEmoteSetDataRemoveEmote"}
			<Minus />
			<span class="text">
				Removed emote
				{@render emoteLink(event.data.removedEmote)}
				from
				{@render emoteSetLink(event.target)}
				{@render userLink(event.actor)}
			</span>
		{:else if event.data.__typename === "EventEmoteSetDataRenameEmote"}
			<PencilSimple />
			<span class="text">
				Renamed emote
				{@render emoteLink(event.data.renamedEmote)}
				in
				{@render emoteSetLink(event.target)}
				from
				<b>{event.data.oldAlias}</b>
				to
				<b>{event.data.newAlias}</b>
				{@render userLink(event.actor)}
			</span>
		{:else if event.data.__typename === "EventEmoteSetDataDelete"}
			<FolderSimple />
			<span class="text">
				{@render emoteSetLink(event.target)}
				deleted
				{@render userLink(event.actor)}
			</span>
		{/if}
		<span class="time">
			<FromNow date={moment(event.createdAt)} />
		</span>
	</div>
</IconContext>

<style lang="scss">
	.event {
		display: grid;
		grid-template-areas: "icon text" ". time";
		justify-content: start;
		align-items: center;
		row-gap: 0.5rem;

		padding: 1rem;
		background-color: var(--bg-medium);
		border-radius: 0.5rem;

		font-size: 0.75rem;
		font-weight: 500;

		.text {
			grid-area: text;
		}

		a {
			color: var(--text);
		}

		.time {
			grid-area: time;
			color: var(--text-light);
		}
	}

	.emote-link {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;

		.emote-name {
			font-weight: 700;

			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
			max-width: 15rem;
		}
	}
</style>
