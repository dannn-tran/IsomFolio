module IsomFolio.Core.Metadata.TagTree

type TagNodeKind = Tagged | Ghost

type TagNode = {
    Segment  : string
    FullPath : string
    Kind     : TagNodeKind
    Children : TagNode list
}

let rec private insertAt (prefix: string) (nodes: TagNode list) (parts: string list) : TagNode list =
    match parts with
    | [] -> nodes
    | [leaf] ->
        let fullPath = if prefix = "" then leaf else prefix + "/" + leaf
        match nodes |> List.tryFindIndex (fun n -> n.Segment = leaf) with
        | Some i ->
            nodes |> List.mapi (fun j n -> if j = i then { n with Kind = Tagged } else n)
        | None ->
            nodes @ [ { Segment = leaf; FullPath = fullPath; Kind = Tagged; Children = [] } ]
    | head :: rest ->
        let headFull = if prefix = "" then head else prefix + "/" + head
        match nodes |> List.tryFindIndex (fun n -> n.Segment = head) with
        | Some i ->
            let n = nodes.[i]
            let newChildren = insertAt headFull n.Children rest
            nodes |> List.mapi (fun j x -> if j = i then { x with Children = newChildren } else x)
        | None ->
            let newChildren = insertAt headFull [] rest
            nodes @ [ { Segment = head; FullPath = headFull; Kind = Ghost; Children = newChildren } ]

// Remove Ghost nodes that have become childless (bottom-up).
let rec private pruneGhosts (nodes: TagNode list) : TagNode list =
    nodes
    |> List.map (fun n -> { n with Children = pruneGhosts n.Children })
    |> List.filter (fun n -> n.Kind = Tagged || not n.Children.IsEmpty)

let buildTree (tags: string list) : TagNode list =
    tags
    |> List.sort
    |> List.fold (fun acc tag ->
        let parts = tag.Split('/') |> Array.toList
        insertAt "" acc parts)
        []

let rec flattenTree (nodes: TagNode list) : string list =
    nodes |> List.collect (fun n ->
        let self = if n.Kind = Tagged then [ n.FullPath ] else []
        self @ flattenTree n.Children)

let addTag (fullPath: string) (nodes: TagNode list) : TagNode list =
    let parts = fullPath.Trim('/').Split('/') |> Array.toList
    insertAt "" nodes parts

let rec private removeAt (fullPath: string) (demote: bool) (nodes: TagNode list) : TagNode list =
    match nodes |> List.tryFindIndex (fun n -> n.FullPath = fullPath) with
    | Some i ->
        let n = nodes.[i]
        if n.Children.IsEmpty then
            nodes |> List.removeAt i
        elif demote then
            nodes |> List.mapi (fun j x -> if j = i then { x with Kind = Ghost } else x)
        else
            nodes |> List.removeAt i
    | None ->
        nodes |> List.map (fun n -> { n with Children = removeAt fullPath demote n.Children })

// Demotes to Ghost if the node has children; deletes if it is a leaf.
// Cleans up any Ghost ancestors that become childless as a result.
let removeTag (fullPath: string) (nodes: TagNode list) : TagNode list =
    removeAt fullPath true nodes |> pruneGhosts

// Removes the node and its entire subtree.
// Cleans up any Ghost ancestors that become childless as a result.
let removeSubtree (fullPath: string) (nodes: TagNode list) : TagNode list =
    removeAt fullPath false nodes |> pruneGhosts

let rec reTag (fullPath: string) (nodes: TagNode list) : TagNode list =
    nodes |> List.map (fun n ->
        if n.FullPath = fullPath then { n with Kind = Tagged }
        else { n with Children = reTag fullPath n.Children })
