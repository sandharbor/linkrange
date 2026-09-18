https://youtu.be/IGu7ivuy1Ag?si=ff2QkHNfYfXp2eFd

[[person - Niels Rogge]]

[[diff - training vs inference]]

At like one minute, he's working on the [[project - huggingface transformers library]]

At 1:16 he points to the [[B the illustrated transformer]] it goes into more details about [[attention]]

At 5:30 the results of tokenization are what is called [[input id]]s

At 8:00 what does the [[encoder]] do during [[training]]?  It takes the input ids and generates the [[embedding vector]] in the [[embedding table]].  Another name for embedding vectors is [[last hidden states]].  He mentioned that they are hidden [[representation]]

At 9:58 [[dim - transformer sizes]] ... The one with 768 is [[dim - transformer sizes - base]]


[00:10:50](https://www.youtube.com/watch?v=IGu7ivuy1Ag#t=10:50.96) during [[inference]], The [[decoder]] takes in [[decoder input ids]]

At 11:10 at timestamp 1 we pass the [[special token - decoder start token]] which looks like `<s>`

At 11:40, we [[condition]] the [[decoder]] on the [[last hidden states]]

At 13:30 we have a [[language modeling head]], which is a [[linear layer]] or a [[linear transformation]] ... "so basically a [[matrix multiply]]".  We basically map the [[last hidden states]] of the [[decoder]] through that linear layer to create the [[logit]]s, which is a vector of the size of the [[dim - token vocabulary size]] (50,000 in this example).  The logits are basically [[unnormalized scores]].

At maybe 15 the next token is simply the [[logit]] with the highest value

At 17:30 when you [[condition]] the [[decoder]] for the second token, you condition it with the [[last hidden states]] from the [[encoder]] and both of the [[decoder input ids]] (the [[special token - decoder start token]]'s id and the next new token's is)

At 25 he talks about the [[special token - end-of-sequence token]] that is generated at the end.

At 29:30 taking the highest score from the [[logit]] is known as [[greedy decoding]], but there are other approaches as well such as [[beam search]] x [[decoder]]

At 34:30 when training something like ChatGPT the [[decoder input ids]] are actually the [[label]]s, shifted one to the right with the [[special token - decoder start token]] prepended.

At 39:45 the [[language modeling head]] is a [[learnable matrix]]... in other words, the values in the 768 x 50,000 ( [[dim - token vocabulary size]]) matrix were learned by [[backpropagation]].

At 41 it calculates the [[cross-entropy]] [[training loss]] between the [[label]] and the model's [[prediction]] ([[logit]]).  The final loss is the average, or mean, of the cross-entropy loss between all of the labels and all of the predictions.

At 42:30 we do things in [[batch]], so to get [[fixed-size sequences]] we will use [[padding]], and specifically a [[special token - padding token]].  512 seems to be a common fixed-size sequence length X [[moc - sequences]] [[dim - sequence length]] [[con - fixed-size sequences use padding]]

At 43:40 to avoid calculating [[cross-entropy]] loss for [[special token - padding token]]s, we use the value -100 for the [[label]]s

At 4420 if the labels contained the -100 value value, then for each of those the [[special token - padding token]] Will be added to the [[decoder input ids]]

At 47 The idea of providing the label data as a target sequence for [[training]] Is called [[teacher forcing]]

At 47 since all of the [[decoder input ids]] are provided together in one long sequence during training, you need to ensure that the [[decoder]] is not able to cheat and look beyond the token it is trying to calculate.  To do that you use a [[causal attention mask]] x [[masked self-attention]]



### Prompts

#flashcards/transformers

 From 29

 During [[inference]], what is the name of the thing that comes after you do the [[linear transformation]] on the [[last hidden states]] of the [[decoder]]?::: the [[logit]]

From 29

What is the [[logit]] in the [[decoder]] the result of?::: applying a [[linear layer]] to the [[last hidden states]] of the the [[decoder]]

From 41

During the training of something like ChatGPT, what is done with the [[logit]] value that is output by the [[decoder]]?::: it runs the [[cross-entropy]] [[loss function]] against the [[logit]] and the label

From 45

For [[training]], explain how the [[decoder]] is [[condition]]ed to allow for determining the [[training loss]] on sequence to sequence prediction using only a single [[forward pass]] X [[dim - number of forward passes]]?::: you have a set of target [[label]]s that describe the entire sequence, from which the [[decoder input ids]] (which are tokens for the entire target sequence).  The conditioning consists of combining the [[last hidden states]] of the [[encoder]] (the [[embedding vector]]),
