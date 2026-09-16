What is the meaning of the self in multi-headed self attention?

<hr class="__chatgpt_plugin">

role::assistant

In the context of multi-headed self attention in transformer models (like BERT or GPT), "self" refers to the fact that the attention scores are computed from the [[input sequence]] itself. 

In other words, each element in the input sequence has the opportunity to 'attend to' or focus on other elements in the same sequence. This is in contrast to other forms of attention where a sequence might attend to a different sequence. 

The term "self" signifies that the model is paying attention to its own input sequence to determine the importance of words in the sequence, and how they relate to each other. 

The "multi-headed" part refers to the fact that this attention process is done multiple times (in parallel) with different learned linear transformations of the input, allowing the model to focus on different features in the data. Each of these parallel attention processes is known as a 'head'.

<hr class="__chatgpt_plugin">

role::user

#flashcards/transformers

What does the self in self attention mean?::: The tokens being attended to come from the input sequence, not from another sequence
